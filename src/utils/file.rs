//! 文件操作工具

use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::Path;
use crate::error::Result;

/// 如果目录不存在则创建目录
pub fn create_dir(dirname: &str) -> Result<()> {
    if !Path::new(dirname).exists() {
        fs::create_dir_all(dirname)?;
    }
    Ok(())
}

/// 删除目录
pub fn remove_dir(dirname: &str) -> Result<()> {
    if Path::new(dirname).exists() {
        fs::remove_dir(dirname)?;
    }
    Ok(())
}

/// 将多个文件追加到目标文件
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

/// 删除文件
pub fn delete_file(path: &str) -> Result<()> {
    if Path::new(path).exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}