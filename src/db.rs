//! Database module: PostgreSQL connection management and query operations
use anyhow::{Context, Result};
use chrono::DateTime;
use sqlx::{PgPool, Row, Transaction};
use uuid::Uuid;

pub async fn init_pool(db_url: &str) -> Result<PgPool> {
    PgPool::connect(db_url).await.context("Failed to connect to PostgreSQL")
}

/// Run migrations by executing the SQL schema statements individually.
/// sqlx::query() only supports single statements, so we split by ';'.
pub async fn run_migrations(pool: &PgPool) -> Result<()> {
    let migration_sql = include_str!("../migrations/001_init.sql");
    
    // Start a transaction for atomicity
    let mut tx = pool.begin().await?;
    
    // Split by semicolon and execute each non-empty statement
    for stmt in migration_sql.split(';') {
        let trimmed = stmt.trim();
        if !trimmed.is_empty() {
            sqlx::query(trimmed)
                .execute(&mut *tx)
                .await
                .context(format!("Failed to execute migration statement: {}", &trimmed[..trimmed.len().min(100)]))?;
        }
    }
    
    tx.commit().await.context("Failed to commit migrations")?;
    Ok(())
}

pub async fn ensure_drive(pool: &PgPool, fs_uuid: &str) -> Result<Uuid> {
    let row = sqlx::query(
        "INSERT INTO drives (fs_uuid) VALUES ($1) ON CONFLICT (fs_uuid) DO UPDATE SET last_scanned_at = NOW() RETURNING id"
    )
    .bind(fs_uuid)
    .fetch_one(pool)
    .await?;
    Ok(row.get::<Uuid, _>("id"))
}

pub async fn get_existing_file_mtime(
    pool: &PgPool,
    drive_id: Uuid,
    relative_path: &str,
) -> Result<Option<DateTime<chrono::Utc>>> {
    let row = sqlx::query_scalar::<_, Option<i64>>(
        "SELECT EXTRACT(EPOCH FROM modified_date)::bigint FROM files WHERE drive_id = $1 AND relative_path = $2"
    )
    .bind(drive_id)
    .bind(relative_path)
    .fetch_optional(pool)
    .await?;
    
    Ok(row.and_then(|epoch_opt| epoch_opt.and_then(|e| DateTime::from_timestamp(e, 0))))
}

pub async fn count_files_by_size(pool: &PgPool, size_bytes: i64) -> Result<i64> {
    let count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM files WHERE size_bytes = $1"
    )
    .bind(size_bytes)
    .fetch_one(pool)
    .await?;
    Ok(count)
}

pub async fn find_by_size_and_partial_hash(
    pool: &PgPool,
    size_bytes: i64,
    partial_hash: &str,
) -> Result<Vec<(i64, Option<String>)>> {
    let rows = sqlx::query_as::<_, (i64, Option<String>)>(
        "SELECT id, full_hash FROM files WHERE size_bytes = $1 AND partial_hash = $2"
    )
    .bind(size_bytes)
    .bind(partial_hash)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

#[derive(Debug)]
pub struct FileInsert {
    pub file_name: String,
    pub extension: Option<String>,
    pub size_bytes: i64,
    pub relative_path: String,
    pub modified_date: DateTime<chrono::Utc>,
    pub partial_hash: Option<String>,
    pub full_hash: Option<String>,
    pub is_duplicate: bool,
    pub canonical_file_id: Option<i64>,
}

pub async fn bulk_insert_files(
    pool: &PgPool,
    drive_id: Uuid,
    files: Vec<FileInsert>,
) -> Result<()> {
    if files.is_empty() { return Ok(()); }
    
    let mut tx = pool.begin().await?;
    
    for file in &files {
        sqlx::query(
            "INSERT INTO files 
             (drive_id, file_name, extension, size_bytes, relative_path, modified_date, 
              partial_hash, full_hash, is_duplicate, canonical_file_id)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
             ON CONFLICT (drive_id, relative_path) DO UPDATE SET
                 file_name = EXCLUDED.file_name,
                 extension = EXCLUDED.extension,
                 size_bytes = EXCLUDED.size_bytes,
                 modified_date = EXCLUDED.modified_date,
                 partial_hash = EXCLUDED.partial_hash,
                 full_hash = EXCLUDED.full_hash,
                 is_duplicate = EXCLUDED.is_duplicate,
                 canonical_file_id = EXCLUDED.canonical_file_id"
        )
        .bind(drive_id)
        .bind(&file.file_name)
        .bind(&file.extension)
        .bind(file.size_bytes)
        .bind(&file.relative_path)
        .bind(file.modified_date)
        .bind(&file.partial_hash)
        .bind(&file.full_hash)
        .bind(file.is_duplicate)
        .bind(file.canonical_file_id)
        .execute(&mut *tx)
        .await?;
    }
    
    tx.commit().await.context("Failed to commit transaction")?;
    Ok(())
}
