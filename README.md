# The Crawler 🕷️

**The Crawler** is a high-performance, production-grade CLI tool written in Rust designed to catalog millions of files across external drives into a PostgreSQL database to facilitate deduplication. It employs a **tiered hashing strategy** to minimize I/O and network round-trips, ensuring efficient processing even on large datasets.

## ✨ Key Features

- **Tiered Hashing Strategy**: Dramatically reduces disk I/O by filtering files in stages:
  1. **Size Check**: If a file size is unique, it's marked unique immediately.
  2. **Partial Hash**: If sizes match, it hashes only the first 1KB.
  3. **Full Hash**: A full SHA-256 is performed only as a last resort for potential duplicates.
- **Drive-Centric Identification**: Tracks drives via Filesystem UUIDs. If UUID resolution fails (e.g., on certain NTFS mounts), it uses a path-based hash to ensure drives are distinguished.
- **Resilient Scanning**: Built to handle "real-world" hardware. It logs and skips `Input/output` errors or permission issues without crashing.
- **High Concurrency**: Implements a Producer-Consumer pattern using `Tokio` and `mpsc` channels to maximize CPU and I/O throughput.
- **Incremental Scanning**: Uses modification times (`mtime`) to skip unchanged files during subsequent scans of the same drive.

## 🛠️ Tech Stack

- **Language**: Rust (Edition 2021)
- **Async Runtime**: [Tokio](https://tokio.rs/)
- **Database**: [PostgreSQL](https://www.postgresql.org/) via [`sqlx`](https://github.com/launchbadge/sqlx)
- **File Traversal**: [`ignore`](https://docs.rs/ignore/latest/ignore/) (Parallel walker that respects `.gitignore`)
- **Hashing**: [`sha2`](https://docs.rs/sha2/latest/sha2/) (SHA-256)
- **CLI**: [`clap`](https://docs.rs/clap/latest/clap/)
- **Logging**: [`tracing`](https://docs.rs/tracing/latest/tracing/)

## 🚀 Getting Started

### Prerequisites
- **OS**: Linux (Ubuntu 24.04+ recommended)
- **Rust**: Latest stable toolchain via `rustup`
- **Database**: PostgreSQL 16+
- **System Tools**: `util-linux` (for `blkid`)

### Installation

```bash
# Clone the repository
git clone https://github.com/yourusername/thecrawler.git
cd thecrawler

# Build in release mode for maximum performance
cargo build --release
```

### Database Setup

```sql
-- Create a dedicated user and database
CREATE USER thecrawler WITH PASSWORD 'your_secure_password';
CREATE DATABASE thecrawler OWNER thecrawler;
GRANT ALL PRIVILEGES ON DATABASE thecrawler TO thecrawler;
```

### Usage

To ensure drives are identified uniquely, it is recommended to mount each drive to a unique directory (e.g., `/mnt/drive_001`, `/mnt/drive_002`).

```bash
./target/release/thecrawler \
    --path /mnt/drive_001 \
    --db-url "postgresql://thecrawler:your_secure_password@localhost/thecrawler" \
    --workers 4 \
    --batch-size 500
```

## 📂 Project Layout

```text
.
├── Cargo.toml              # Dependencies and project config
├── migrations/             # SQL Schema
│   └── 001_init.sql        # Table and Index definitions
├── src/                    # Rust source code
│   ├── main.rs             # Entry point & CLI parsing
│   ├── crawler.rs          # Parallel walking and tiered hashing logic
│   ├── db.rs               # Database operations
│   └── utils.rs            # Utility functions (UUID resolution)
└── README.md               # Documentation
```

## 🔍 The Tiered Hashing Logic

To avoid the "I/O Bottleneck" of reading terabytes of data, The Crawler uses the following flow:

| Step | Check | Action if Match Found | Action if No Match |
| :--- | :--- | :--- | :--- |
| 1 | **mtime** | Skip file (Unchanged) | Proceed to Step 2 |
| 2 | **Size** | Proceed to Step 3 | Mark Unique $\rightarrow$ Done |
| 3 | **1KB Hash** | Proceed to Step 4 | Mark Unique $\rightarrow$ Done |
| 4 | **Full Hash** | Mark Duplicate $\rightarrow$ Done | Mark Unique $\rightarrow$ Done |

## 📈 Performance Tips

- **Mount Options**: For NTFS drives, using the native `ntfs` driver (instead of `ntfs-3g`) and adding the `-o noatime` flag can significantly increase scan speeds.
- **Worker Count**: Set `--workers` to match your CPU core count for hashing, but be mindful of disk I/O limits (too many workers on a single HDD can cause head thrashing).
- **Batch Size**: Larger batch sizes (e.g., 500-1000) reduce the number of database transactions, increasing throughput.

## 📄 License
This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
