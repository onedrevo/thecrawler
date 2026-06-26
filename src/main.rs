//! The Crawler - High-Performance File Indexer
//  - TODO: Add support for dry-run mode to preview scans without writing to DB
mod db;
mod crawler;
mod utils;

use anyhow::{Context, Result};
use clap::Parser;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::Mutex;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
#[command(name = "thecrawler", version, about, long_about = None)]
struct Args {
    /// Root directory to scan (mount point of the drive)
    #[arg(short, long)]
    path: PathBuf,

    /// PostgreSQL connection string
    #[arg(short, long)]
    db_url: String,

    /// Number of consumer workers (default: 4)
    #[arg(short, long, default_value_t = 4)]
    workers: usize,

    /// Channel buffer size (default: 1000)
    #[arg(long, default_value_t = 1000)]
    channel_buffer: usize,

    /// Bulk insert batch size (default: 500)
    #[arg(long, default_value_t = 500)]
    batch_size: usize,

    /// Follow symlinks (default: false)
    #[arg(long, action = clap::ArgAction::SetTrue)]
    follow_symlinks: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().with_env_filter(
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
    ).init();

    let args = Args::parse();
    let root_path = args.path.canonicalize().context("Could not resolve scan path")?;
    if !root_path.is_dir() { anyhow::bail!("Path '{}' is not a directory", root_path.display()); }

    info!("Starting The Crawler");
    info!("Root path: {}", root_path.display());
    
    let pool = db::init_pool(&args.db_url).await?;
    info!("Connected to PostgreSQL database");
    
    db::run_migrations(&pool).await?;
    info!("Database migrations verified/applied");

    let fs_uuid = utils::get_filesystem_uuid(&root_path)?;
    info!("Filesystem UUID: {}", fs_uuid);

    let drive_id = db::ensure_drive(&pool, &fs_uuid).await?;
    info!("Drive ID: {}", drive_id);

    // Create the channel
    let (tx, rx) = mpsc::channel::<crawler::FileMetadata>(args.channel_buffer);

    // Wrap receiver in Arc<Mutex> to share it safely among multiple workers
    let rx_shared = Arc::new(Mutex::new(rx));

    // Spawn Producer
    let root_path_clone = root_path.clone();
    let follow_symlinks = args.follow_symlinks;
    let tx_for_producer = tx.clone(); 
    let producer_handle = tokio::spawn(async move {
        crawler::walk_directory(root_path_clone, tx_for_producer, follow_symlinks).await;
    });

    // Spawn Consumers
    let pool_arc = Arc::new(pool);
    let mut consumer_handles = Vec::with_capacity(args.workers);

    for worker_id in 0..args.workers {
        let pool_worker = Arc::clone(&pool_arc);
        let rx_worker = Arc::clone(&rx_shared); // Clone the shared pointer, not the receiver itself
        let batch_size = args.batch_size;
        let root_path_worker = root_path.clone();

        let handle = tokio::spawn(async move {
            crawler::process_files(worker_id, rx_worker, pool_worker, drive_id, &root_path_worker, batch_size).await
        });
        consumer_handles.push(handle);
    }

    // Wait for producer to finish
    let _ = producer_handle.await?;
    info!("Producer finished scanning");
    
    // Drop the original sender so consumers know when to stop
    drop(tx);

    // Wait for all consumers to finish processing remaining items
    for handle in consumer_handles {
        handle.await??;
    }

    info!("All consumers finished. Scan complete!");
    Ok(())
}
