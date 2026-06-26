# The Crawler 🕷️

**The Crawler** is a high-performance, production-grade CLI tool written in Rust that catalogs millions of files across external drives into a PostgreSQL database to facilitate deduplication. It employs a **tiered hashing strategy** to minimize I/O and network round-trips, ensuring efficient processing even on large datasets.

## ✨ Features

- **Tiered Hashing Strategy**: Minimizes disk I/O by checking file size first, then partial hash (first 1KB), and only performing full SHA-256 hashes when necessary.
- **Resilient & Robust**: Never crashes on permission errors or vanished files; logs issues and continues scanning.
- **Drive Identification**: Tracks drives by Filesystem UUID, ensuring data integrity even if mount points change.
- **High Concurrency**: Uses a producer-consumer pattern with Tokio async runtime for high throughput.
- **Resumable Scans**: Skips unchanged files based on modification time, allowing incremental updates.
- **Parallel Traversal**: Uses the `ignore` crate for fast, `.gitignore`-aware directory walking.

## 🛠️ Tech Stack

- **Language**: Rust (Edition 2021)
- **Async Runtime**: Tokio
- **Database**: PostgreSQL via `sqlx`
- **File Traversal**: `ignore` crate
- **Hashing**: `sha2` (SHA-256)
- **CLI**: `clap`
- **Logging**: `tracing`

## 🚀 Quick Start

### Prerequisites

- Rust toolchain (`rustup`)
- PostgreSQL 16+
- Linux (Ubuntu 24.04+ recommended)

### Installation

```bash
# Clone the repository
git clone https://github.com/yourusername/thecrawler.git
cd thecrawler

# Build in release mode
cargo build --release

Database Setup
sql

-- Create user and database
CREATE USER thecrawler WITH PASSWORD 'thecrawler_secure_pass_123';
CREATE DATABASE thecrawler OWNER thecrawler;
GRANT ALL PRIVILEGES ON DATABASE thecrawler TO thecrawler;

Usage
bash

./target/release/thecrawler \
    --path /mnt/usb_vol01 \
    --db-url "postgresql://thecrawler:thecrawler_secure_pass_123@localhost/thecrawler" \
    --workers 4

📂 Project Structure
text

.
├── Cargo.toml              # Project manifest
├── migrations/             # SQL schema migrations
│   └── 001_init.sql        # Initial database schema
├── src/                    # Rust source code
│   ├── main.rs             # Entry point & CLI parsing
│   ├── crawler.rs          # File traversal & processing logic
│   ├── db.rs               # Database operations
│   └── utils.rs            # Utility functions (UUID resolution)
└── README.md               # This file

🔍 Deduplication Logic

    Metadata Check: Retrieve size, mtime, and path.
    Resumability Check: Skip if drive_id + relative_path exists with same mtime.
    Size Uniqueness: If no other file has this size, mark as unique (no hashing needed).
    Partial Hash: SHA-256 of first 1024 bytes. If unique by partial hash, stop.
    Full Hash: SHA-256 of entire file only if partial hash matches another file.

📊 Database Schema

The tool creates two main tables: drives and files, with optimized indexes for fast lookups during the tiered hashing process.
🤝 Contributing

Contributions are welcome! Please feel free to submit a Pull Request.
📄 License

This project is licensed under the MIT License - see the LICENSE [blocked] file for details. EOF


### Step 2: Add an MIT License

The MIT License is permissive and popular for open-source tools.

```bash
curl -O https://raw.githubusercontent.com/mit-license/master/LICENSE

Note: If curl fails, you can create it manually:
bash

cat > LICENSE << 'EOF'
MIT License

Copyright (c) 2026 Plato

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
