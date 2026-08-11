//! Общие файловые утилиты, не привязанные к конкретной субкоманде.

use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};

/// Удаляет `start` и всех его родителей, пока они пусты, не поднимаясь выше `root`
/// (сам `root` никогда не удаляется). Возвращает список удалённых директорий.
pub fn remove_empty_ancestors(root: &Path, start: &Path) -> Result<Vec<PathBuf>> {
    let mut removed = Vec::new();
    let mut dir = start.to_path_buf();

    while dir != root && dir.starts_with(root) {
        let is_empty = fs::read_dir(&dir)?.next().is_none();
        if !is_empty {
            break;
        }
        fs::remove_dir(&dir)?;
        removed.push(dir.clone());

        match dir.parent() {
            Some(parent) => dir = parent.to_path_buf(),
            None => break,
        }
    }

    Ok(removed)
}

/// Человекочитаемый размер (B/KB/MB/...).
pub fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit = 0;
    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} {}", UNITS[unit])
    } else {
        format!("{size:.1} {}", UNITS[unit])
    }
}
