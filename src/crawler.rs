//! Crawler module: File traversal and processing logic
use anyhow::{Context, Result};
use chrono::DateTime;
use ignore::{WalkBuilder, WalkState};
use indicatif::{ProgressBar, ProgressStyle};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::db::{self, FileInsert};

#[derive(Debug, Clone)]
pub struct FileMetadata {
    pub path: PathBuf,
    pub size_bytes: u64,
    pub modified_date: DateTime<chrono::Utc>,
    pub file_name: String,
    pub extension: Option<String>,
}

pub async fn walk_directory(
    root_path: PathBuf,
    tx: mpsc::Sender<FileMetadata>,
    follow_symlinks: bool,
) {
    let progress = ProgressBar::new_spinner();
    // Fixed template: removed invalid {} and used valid indicatif keys
    let _ = progress.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.green} {msg}")
            .unwrap_or_else(|_| ProgressStyle::default_spinner())
    );
    progress.set_message("Scanning files...");

    let walker = WalkBuilder::new(&root_path)
        .follow_links(follow_symlinks)
        .build_parallel();

    walker.run(|| {
        let tx_clone = tx.clone();
        Box::new(move |entry_result| {
            match entry_result {
                Ok(entry) => {
                    if entry.file_type().map_or(false, |ft| ft.is_file()) {
                        let path = entry.path().to_path_buf();
                        match std::fs::symlink_metadata(&path) {
                            Ok(metadata) => {
                                if metadata.file_type().is_symlink() && !follow_symlinks {
                                    return WalkState::Continue;
                                }

                                let size_bytes = metadata.len();
                                let modified_date = metadata
                                    .modified()
                                    .ok()
                                    .map(|t| DateTime::<chrono::Utc>::from(t))
                                    .unwrap_or_else(|| chrono::Utc::now());

                                let file_name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                                let extension = path.extension().map(|e| e.to_string_lossy().to_string());

                                let meta = FileMetadata { path, size_bytes, modified_date, file_name, extension };

                                if tx_clone.blocking_send(meta).is_err() {
                                    error!("Channel closed while walking directory");
                                    return WalkState::Quit;
                                }
                            }
                            Err(e) => {
                                if e.kind() != std::io::ErrorKind::NotFound {
                                    warn!("Could not read metadata for {:?}: {}", entry.path().display(), e);
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    if e.io_error().map_or(true, |io_err| io_err.kind() != std::io::ErrorKind::NotFound) {
                        warn!("Walk error: {:?}", e);
                    }
                }
            }
            WalkState::Continue
        })
    });

    progress.finish_with_message("Scanning complete");
    info!("Producer finished scanning directory");
}

// Updated signature to accept Arc<Mutex<Receiver>>
pub async fn process_files(
    worker_id: usize,
    rx: Arc<Mutex<mpsc::Receiver<FileMetadata>>>,
    pool: Arc<PgPool>,
    drive_id: Uuid,
    root_path: &Path,
    batch_size: usize,
) -> Result<()> {
    info!("Worker {} started", worker_id);
    let mut buffer: Vec<FileInsert> = Vec::with_capacity(batch_size);
    let mut processed_count = 0u64;

    let worker_progress = ProgressBar::new_spinner();
    // Fixed template: removed invalid {} and used valid indicatif keys
    let _ = worker_progress.set_style(
        ProgressStyle::default_spinner()
            .template("[Worker {id}] {spinner:.blue} processed: {msg}")
            .unwrap_or_else(|_| ProgressStyle::default_spinner())
    );
    
    // Set the worker ID in the progress bar state if supported, otherwise just use msg
    // indicatif doesn't have a built-in 'id' key for spinners easily without custom state.
    // Let's simplify the template to avoid any potential issues.
    let _ = worker_progress.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.blue} Worker {id}: {msg}")
            .unwrap_or_else(|_| ProgressStyle::default_spinner())
    );
    
    // Actually, the simplest fix is to not use {} in the template string at all for static text.
    // Let's use a very simple template.
    let _ = worker_progress.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.blue} {msg}")
            .unwrap_or_else(|_| ProgressStyle::default_spinner())
    );

    loop {
        // Lock the receiver to try_recv
        let mut rx_lock = rx.lock().await;
        
        match rx_lock.try_recv() {
            Ok(meta) => {
                drop(rx_lock); // Release lock immediately after receiving
                
                let relative_path = meta.path.strip_prefix(root_path).unwrap_or(&meta.path).to_string_lossy().to_string();
                
                match process_single_file(&pool, drive_id, &meta, &relative_path).await {
                    Ok(Some(record)) => {
                        buffer.push(record);
                        processed_count += 1;
                        if buffer.len() >= batch_size {
                            flush_buffer(&pool, drive_id, &mut buffer).await?;
                        }
                    }
                    Ok(None) => {} // Skipped unchanged
                    Err(e) => error!("Worker {} failed to process {:?}: {}", worker_id, meta.path, e),
                }

                if processed_count % 100 == 0 {
                    worker_progress.set_message(format!("{} processed", processed_count));
                }
            }
            Err(mpsc::error::TryRecvError::Empty) => {
                drop(rx_lock); // Release lock before sleeping
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
            Err(mpsc::error::TryRecvError::Disconnected) => {
                drop(rx_lock);
                break;
            }
        }
    }

    if !buffer.is_empty() { flush_buffer(&pool, drive_id, &mut buffer).await?; }
    
    worker_progress.finish_with_message(format!("Worker {} finished: {} processed", worker_id, processed_count));
    Ok(())
}

async fn process_single_file(
    pool: &PgPool,
    drive_id: Uuid,
    meta: &FileMetadata,
    relative_path: &str,
) -> Result<Option<FileInsert>> {
    if let Some(existing_mtime) = db::get_existing_file_mtime(pool, drive_id, relative_path).await? {
        if existing_mtime == meta.modified_date { return Ok(None); }
    }

    let size_bytes = meta.size_bytes as i64;
    let count_by_size = db::count_files_by_size(pool, size_bytes).await?;

    if count_by_size == 0 {
        return Ok(Some(FileInsert {
            file_name: meta.file_name.clone(), extension: meta.extension.clone(), size_bytes,
            relative_path: relative_path.to_string(), modified_date: meta.modified_date,
            partial_hash: None, full_hash: None, is_duplicate: false, canonical_file_id: None,
        }));
    }

    let partial_hash = calculate_partial_hash(&meta.path)?;
    let potential_duplicates = db::find_by_size_and_partial_hash(pool, size_bytes, &partial_hash).await?;

    if potential_duplicates.is_empty() {
        return Ok(Some(FileInsert {
            file_name: meta.file_name.clone(), extension: meta.extension.clone(), size_bytes,
            relative_path: relative_path.to_string(), modified_date: meta.modified_date,
            partial_hash: Some(partial_hash), full_hash: None, is_duplicate: false, canonical_file_id: None,
        }));
    }

    let full_hash = calculate_full_hash(&meta.path)?;
    for (existing_id, existing_full_hash) in &potential_duplicates {
        if let Some(ref existing_hash) = existing_full_hash {
            if *existing_hash == full_hash {
                return Ok(Some(FileInsert {
                    file_name: meta.file_name.clone(), extension: meta.extension.clone(), size_bytes,
                    relative_path: relative_path.to_string(), modified_date: meta.modified_date,
                    partial_hash: Some(partial_hash), full_hash: Some(full_hash), is_duplicate: true, canonical_file_id: Some(*existing_id),
                }));
            }
        }
    }

    Ok(Some(FileInsert {
        file_name: meta.file_name.clone(), extension: meta.extension.clone(), size_bytes,
        relative_path: relative_path.to_string(), modified_date: meta.modified_date,
        partial_hash: Some(partial_hash), full_hash: Some(full_hash), is_duplicate: false, canonical_file_id: None,
    }))
}

fn calculate_partial_hash(path: &Path) -> Result<String> {
    let mut file = File::open(path).context(format!("Failed to open file for partial hash: {:?}", path))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 1024];
    if let Ok(bytes_read) = file.read(&mut buffer) { hasher.update(&buffer[..bytes_read]); }
    else { return Err(anyhow::anyhow!("Failed to read file for partial hash")); }
    Ok(format!("{:x}", hasher.finalize()))
}

fn calculate_full_hash(path: &Path) -> Result<String> {
    let path_clone = path.to_path_buf();
    tokio::task::block_in_place(|| {
        let mut file = File::open(&path_clone).context(format!("Failed to open file for full hash: {:?}", path_clone))?;
        let mut hasher = Sha256::new();
        let mut buffer = [0u8; 8192];
        loop {
            match file.read(&mut buffer) {
                Ok(0) => break,
                Ok(bytes_read) => hasher.update(&buffer[..bytes_read]),
                Err(e) => return Err(anyhow::anyhow!("Failed to read file for full hash: {:?}", e)),
            }
        }
        Ok(format!("{:x}", hasher.finalize()))
    })
}

async fn flush_buffer(pool: &PgPool, drive_id: Uuid, buffer: &mut Vec<FileInsert>) -> Result<()> {
    if buffer.is_empty() { return Ok(()); }
    let batch = std::mem::take(buffer);
    db::bulk_insert_files(pool, drive_id, batch).await?;
    Ok(())
}
