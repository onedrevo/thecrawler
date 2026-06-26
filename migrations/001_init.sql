CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

CREATE TABLE IF NOT EXISTS drives (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    fs_uuid TEXT UNIQUE NOT NULL,
    label TEXT,
    last_scanned_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS files (
    id BIGSERIAL PRIMARY KEY,
    drive_id UUID REFERENCES drives(id) ON DELETE CASCADE,
    file_name TEXT NOT NULL,
    extension TEXT,
    size_bytes BIGINT NOT NULL CHECK (size_bytes >= 0),
    relative_path TEXT NOT NULL,
    modified_date TIMESTAMPTZ NOT NULL,
    partial_hash TEXT,
    full_hash TEXT,
    is_duplicate BOOLEAN DEFAULT FALSE,
    canonical_file_id BIGINT REFERENCES files(id) ON DELETE SET NULL,
    UNIQUE(drive_id, relative_path)
);

CREATE INDEX IF NOT EXISTS idx_files_size ON files(size_bytes);
CREATE INDEX IF NOT EXISTS idx_files_partial ON files(partial_hash) WHERE partial_hash IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_files_full ON files(full_hash) WHERE full_hash IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_files_mtime ON files(drive_id, modified_date);
CREATE INDEX IF NOT EXISTS idx_files_size_partial ON files(size_bytes, partial_hash) WHERE partial_hash IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_files_canonical ON files(canonical_file_id) WHERE canonical_file_id IS NOT NULL;
