//! Utility module: Platform-specific filesystem UUID resolution
use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;
use std::fs;
use tracing::warn;
use sha2::{Digest, Sha256};

pub fn get_filesystem_uuid(path: &Path) -> Result<String> {
    #[cfg(target_os = "linux")]
    return get_fs_uuid_linux(path);
    
    #[cfg(not(target_os = "linux"))]
    anyhow::bail!("Only Linux is supported for UUID resolution in this version")
}

#[cfg(target_os = "linux")]
fn get_fs_uuid_linux(path: &Path) -> Result<String> {
    // Strategy 1: Try blkid on the path directly
    if let Ok(uuid) = try_blkid_on_path(path) {
        return Ok(uuid);
    }

    // Strategy 2: Find the underlying device from /proc/self/mountinfo and run blkid on that
    if let Some(device) = find_device_for_mount_point(path)? {
        if let Ok(uuid) = try_blkid_on_device(&device) {
            return Ok(uuid);
        }
    }

    // Strategy 3: Fallback to using the mount point path as a unique identifier (less ideal but works)
    warn!("Could not resolve filesystem UUID via blkid. Using path hash as fallback.");
    let path_str = path.to_string_lossy();
    let mut hasher = Sha256::new();
    hasher.update(path_str.as_bytes());
    let result = hasher.finalize();
    Ok(format!("{:x}", result))
}

#[cfg(target_os = "linux")]
fn try_blkid_on_path(path: &Path) -> Result<String> {
    let output = Command::new("blkid")
        .arg("-s")
        .arg("UUID")
        .arg("-o")
        .arg("value")
        .arg(path)
        .output()
        .context("Failed to execute blkid")?;

    if !output.status.success() {
        return Err(anyhow::anyhow!("blkid failed on path"));
    }

    let uuid = String::from_utf8(output.stdout)
        .context("Invalid UTF-8 in blkid output")?
        .trim()
        .to_string();

    if uuid.is_empty() {
        return Err(anyhow::anyhow!("blkid returned empty UUID for path"));
    }

    Ok(uuid)
}

#[cfg(target_os = "linux")]
fn try_blkid_on_device(device: &str) -> Result<String> {
    let output = Command::new("blkid")
        .arg("-s")
        .arg("UUID")
        .arg("-o")
        .arg("value")
        .arg(device)
        .output()
        .context(format!("Failed to execute blkid on device {}", device))?;

    if !output.status.success() {
        return Err(anyhow::anyhow!("blkid failed on device {}", device));
    }

    let uuid = String::from_utf8(output.stdout)
        .context("Invalid UTF-8 in blkid output")?
        .trim()
        .to_string();

    if uuid.is_empty() {
        return Err(anyhow::anyhow!("blkid returned empty UUID for device {}", device));
    }

    Ok(uuid)
}

#[cfg(target_os = "linux")]
fn find_device_for_mount_point(mount_path: &Path) -> Result<Option<String>> {
    let mountinfo = fs::read_to_string("/proc/self/mountinfo")
        .context("Failed to read /proc/self/mountinfo")?;
    
    let canonical_path = mount_path.canonicalize()
        .context(format!("Failed to canonicalize path {:?}", mount_path))?;

    for line in mountinfo.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 10 {
            continue;
        }

        // Field 5 is the mount point, Field 9 is the device
        let mp = parts[5];
        let dev = parts[9];

        // Check if our path starts with this mount point
        if canonical_path.starts_with(mp) {
            return Ok(Some(dev.to_string()));
        }
    }

    Ok(None)
}
