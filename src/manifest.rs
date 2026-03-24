//! Создание JSON-манифеста набора файлов: относительные пути с «/», UTF-8, сортировка по пути.

use crate::cli::ManifestArgs;
use crate::hash;
use anyhow::{Context, Result};
use serde::Serialize;
use std::fs::{File, metadata};
use std::io::BufWriter;
use std::path::{Path, PathBuf};

/// Версия формата JSON-манифеста (поле `version`).
pub const MANIFEST_FORMAT_VERSION: &str = "1";

#[derive(Serialize)]
struct ManifestFile {
    root: String,
    version: String,
    algorithm: String,
    files: Vec<ManifestEntry>,
}

#[derive(Serialize)]
struct ManifestEntry {
    path: String,
    size: u64,
    hash: String,
}

/// Создаёт UTF-8 JSON-файл с описанием файлов под `path`.
pub fn run(args: &ManifestArgs) -> Result<()> {
    let root_input = &args.path;
    let files = hash::collect_files_for_comparison(root_input)
        .with_context(|| format!("не удалось перечислить файлы: {}", root_input.display()))?;

    let root_path = root_base_for_manifest(root_input);
    let root_str = root_to_manifest_string(&root_path);

    let mut entries: Vec<ManifestEntry> = Vec::with_capacity(files.len());
    for (full, rel) in files {
        let path_str = path_to_slash(rel.as_path());
        let size = metadata(&full)
            .with_context(|| format!("metadata: {}", full.display()))?
            .len();
        let hash = hash::hash_file(&full, &args.algo)
            .with_context(|| format!("hash: {}", full.display()))?;
        entries.push(ManifestEntry {
            path: path_str,
            size,
            hash,
        });
    }

    entries.sort_by(|a, b| a.path.cmp(&b.path));

    let manifest = ManifestFile {
        root: root_str,
        version: MANIFEST_FORMAT_VERSION.to_string(),
        algorithm: hash::algo_name(&args.algo).to_string(),
        files: entries,
    };

    let out = File::create(&args.output)
        .with_context(|| format!("создание файла: {}", args.output.display()))?;
    let writer = BufWriter::new(out);
    serde_json::to_writer_pretty(writer, &manifest)
        .with_context(|| format!("запись JSON: {}", args.output.display()))?;

    Ok(())
}

/// Каталог, который считается корнем для поля `root` (для одного файла — родитель).
fn root_base_for_manifest(path: &Path) -> PathBuf {
    if path.is_dir() {
        path.to_path_buf()
    } else {
        path.parent().map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from("."))
    }
}

/// Абсолютный корень для JSON: без префикса Windows `\\?\`, разделитель `/`.
fn root_to_manifest_string(path: &Path) -> String {
    let p = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let mut s = p.to_string_lossy().into_owned();
    if let Some(rest) = s.strip_prefix(r"\\?\") {
        s = rest.to_string();
    }
    s.replace('\\', "/")
}

/// Относительный или абсолютный путь в виде строки с разделителем `/`.
fn path_to_slash(path: &Path) -> String {
    let s: String = path
        .components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/");
    if s.is_empty() {
        ".".to_string()
    } else {
        s
    }
}
