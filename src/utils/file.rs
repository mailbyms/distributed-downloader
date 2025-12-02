//! File operation utilities

use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::Path;
use crate::error::Result;

/// Create directory if it doesn't exist
pub fn create_dir(dirname: &str) -> Result<()> {
    if !Path::new(dirname).exists() {
        fs::create_dir_all(dirname)?;
    }
    Ok(())
}

/// Remove directory
pub fn remove_dir(dirname: &str) -> Result<()> {
    if Path::new(dirname).exists() {
        fs::remove_dir(dirname)?;
    }
    Ok(())
}

/// Append multiple files to a target file
pub fn append_files(src_paths: &[String], target_path: &str) -> Result<()> {
    let mut target_file = File::create(target_path)?;

    for src_path in src_paths {
        let mut src_file = File::open(src_path)?;
        let mut buffer = Vec::new();
        src_file.read_to_end(&mut buffer)?;
        target_file.write_all(&buffer)?;
    }

    Ok(())
}

/// Delete a file
pub fn delete_file(path: &str) -> Result<()> {
    if Path::new(path).exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}