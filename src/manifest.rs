//! JSON-манифест: создание, обновление и проверка.

use crate::cli::{Algo, ManifestCreateArgs, ManifestUpdateArgs, ManifestVerifyArgs};
use crate::hash;
use anyhow::{Context, Result};
use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{File, metadata, read_to_string};
use std::io::BufWriter;
use std::path::{Path, PathBuf};

/// Версия схемы JSON (поле `version`, строка).
pub const MANIFEST_FORMAT_VERSION: &str = "1";

#[derive(Debug, Serialize, Deserialize, Clone)]
struct ManifestFile {
    root: String,
    /// Версия формата манифеста (строка), не путать с `revision`.
    version: String,
    /// Счётчик обновлений (`manifest update` увеличивает на 1).
    #[serde(default)]
    revision: u64,
    /// Время последней записи манифеста (RFC 3339, UTC).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    timestamp: Option<String>,
    algorithm: String,
    files: Vec<ManifestEntry>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct ManifestEntry {
    path: String,
    size: u64,
    hash: String,
}

fn utc_timestamp_rfc3339() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn resolve_manifest_path(root: &Path, manifest: &Option<PathBuf>) -> PathBuf {
    manifest
        .clone()
        .unwrap_or_else(|| root.join("manifest.json"))
}

/// Сравнение путей с учётом канонизации (Windows, относительные пути).
fn paths_point_to_same_file(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(ca), Ok(cb)) => ca == cb,
        _ => a == b,
    }
}

/// Собирает записи о файлах под `root_input` с хешами `algo`.
/// `exclude_file` — не включать этот файл в список (например сам `manifest.json` при create/update).
fn build_file_entries(
    root_input: &Path,
    algo: &Algo,
    exclude_file: Option<&Path>,
) -> Result<Vec<ManifestEntry>> {
    let files = hash::collect_files_for_comparison(root_input)
        .with_context(|| format!("не удалось перечислить файлы: {}", root_input.display()))?;

    let mut entries: Vec<ManifestEntry> = Vec::with_capacity(files.len());
    for (full, rel) in files {
        if let Some(ex) = exclude_file {
            if paths_point_to_same_file(&full, ex) {
                continue;
            }
        }
        let path_str = path_to_slash(rel.as_path());
        let size = metadata(&full)
            .with_context(|| format!("metadata: {}", full.display()))?
            .len();
        let h = hash::hash_file(full.as_path(), algo)
            .with_context(|| format!("hash: {}", full.display()))?;
        entries.push(ManifestEntry {
            path: path_str,
            size,
            hash: h,
        });
    }

    entries.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(entries)
}

fn write_manifest_file(output: &Path, manifest: &ManifestFile) -> Result<()> {
    let out = File::create(output).with_context(|| format!("запись: {}", output.display()))?;
    let writer = BufWriter::new(out);
    serde_json::to_writer_pretty(writer, manifest)
        .with_context(|| format!("сериализация JSON: {}", output.display()))?;
    Ok(())
}

/// Записывает UTF-8 JSON-манифест.
pub fn create(args: &ManifestCreateArgs) -> Result<()> {
    let root_input = &args.path;
    let root_path = root_base_for_manifest(root_input);
    let root_str = root_to_manifest_string(&root_path);
    let entries = build_file_entries(root_input, &args.algo, Some(args.output.as_path()))?;

    let manifest = ManifestFile {
        root: root_str,
        version: MANIFEST_FORMAT_VERSION.to_string(),
        revision: 1,
        timestamp: Some(utc_timestamp_rfc3339()),
        algorithm: hash::algo_name(&args.algo).to_string(),
        files: entries,
    };

    write_manifest_file(&args.output, &manifest)?;
    Ok(())
}

/// Перечитывает манифест, пересчитывает дерево, увеличивает `revision`, обновляет `timestamp`.
pub fn update(args: &ManifestUpdateArgs) -> Result<()> {
    let manifest_path = resolve_manifest_path(&args.path, &args.manifest);

    if !manifest_path.is_file() {
        anyhow::bail!("файл манифеста не найден: {}", manifest_path.display());
    }

    let text = read_to_string(&manifest_path)
        .with_context(|| format!("чтение: {}", manifest_path.display()))?;
    let stored: ManifestFile = serde_json::from_str(&text)
        .with_context(|| format!("разбор JSON: {}", manifest_path.display()))?;

    if stored.version != MANIFEST_FORMAT_VERSION {
        anyhow::bail!(
            "неподдерживаемая версия схемы манифеста: {:?} (ожидается {:?})",
            stored.version,
            MANIFEST_FORMAT_VERSION
        );
    }

    let algo = match &args.algo {
        Some(a) => a.clone(),
        None => hash::algo_from_manifest_name(&stored.algorithm)?,
    };

    let root_path = root_base_for_manifest(&args.path);
    let root_str = root_to_manifest_string(&root_path);
    let entries = build_file_entries(&args.path, &algo, Some(manifest_path.as_path()))?;

    let next_revision = stored.revision.saturating_add(1);
    let manifest = ManifestFile {
        root: root_str,
        version: MANIFEST_FORMAT_VERSION.to_string(),
        revision: next_revision,
        timestamp: Some(utc_timestamp_rfc3339()),
        algorithm: hash::algo_name(&algo).to_string(),
        files: entries,
    };

    write_manifest_file(&manifest_path, &manifest)?;

    println!(
        "Манифест обновлён: {} (revision → {}, timestamp установлен)",
        manifest_path.display(),
        next_revision
    );

    Ok(())
}

/// Сверяет каталог с манифестом. Печатает отчёт; `true`, если всё совпало.
pub fn verify(args: &ManifestVerifyArgs) -> Result<bool> {
    let manifest_path = resolve_manifest_path(&args.path, &args.manifest);

    if !manifest_path.is_file() {
        anyhow::bail!(
            "файл манифеста не найден: {}",
            manifest_path.display()
        );
    }

    let text = read_to_string(&manifest_path)
        .with_context(|| format!("чтение: {}", manifest_path.display()))?;
    let stored: ManifestFile = serde_json::from_str(&text)
        .with_context(|| format!("разбор JSON: {}", manifest_path.display()))?;

    let algo = hash::algo_from_manifest_name(&stored.algorithm)?;

    let on_disk = hash::collect_files_for_comparison(&args.path)
        .with_context(|| format!("не удалось перечислить файлы: {}", args.path.display()))?;

    let mut disk_map: BTreeMap<String, (u64, String)> = BTreeMap::new();
    for (full, rel) in on_disk {
        let key = path_to_slash(rel.as_path());
        let size = metadata(&full)
            .with_context(|| format!("metadata: {}", full.display()))?
            .len();
        let h = hash::hash_file(&full, &algo)
            .with_context(|| format!("hash: {}", full.display()))?;
        disk_map.insert(key, (size, h));
    }

    if let (Ok(root_canon), Ok(man_canon)) = (args.path.canonicalize(), manifest_path.canonicalize()) {
        if let Ok(rel) = man_canon.strip_prefix(&root_canon) {
            disk_map.remove(&path_to_slash(rel));
        }
    }

    let mut manifest_map: BTreeMap<String, (u64, String)> = BTreeMap::new();
    for e in &stored.files {
        manifest_map.insert(e.path.clone(), (e.size, e.hash.clone()));
    }

    let paths_disk: BTreeSet<_> = disk_map.keys().cloned().collect();
    let paths_man: BTreeSet<_> = manifest_map.keys().cloned().collect();

    let only_manifest: Vec<_> = paths_man.difference(&paths_disk).cloned().collect();
    let only_disk: Vec<_> = paths_disk.difference(&paths_man).cloned().collect();
    let common: Vec<_> = paths_man.intersection(&paths_disk).cloned().collect();

    let mut identical: Vec<&str> = Vec::new();
    let mut size_mismatch: Vec<(&str, u64, u64)> = Vec::new();
    let mut hash_mismatch: Vec<&str> = Vec::new();

    for p in &common {
        let (ms, mh) = manifest_map.get(p).unwrap();
        let (ds, dh) = disk_map.get(p).unwrap();
        if ms == ds && mh == dh {
            identical.push(p.as_str());
        } else {
            if ms != ds {
                size_mismatch.push((p.as_str(), *ms, *ds));
            }
            if mh != dh {
                hash_mismatch.push(p.as_str());
            }
        }
    }

    let all_ok = only_manifest.is_empty() && only_disk.is_empty() && size_mismatch.is_empty() && hash_mismatch.is_empty();

    println!("Манифест: {}", manifest_path.display());
    println!("Корень в манифесте (поле root): {}", stored.root);
    println!("Алгоритм в манифесте: {}", stored.algorithm);
    println!("Версия схемы (поле version): {}", stored.version);
    if stored.revision > 0 {
        println!("Ревизия манифеста (поле revision): {}", stored.revision);
    }
    if let Some(ref ts) = stored.timestamp {
        println!("Метка времени (поле timestamp): {ts}");
    }
    println!();

    if all_ok {
        println!("Содержимое каталога совпадает с манифестом (размер и хеш для всех {} файлов).", identical.len());
        return Ok(true);
    }

    println!("Есть отличия относительно манифеста.");
    println!();

    if !identical.is_empty() {
        println!("Совпадают (размер и хеш) — {} файл(ов):", identical.len());
        for p in &identical {
            println!("  ✔ {p}");
        }
        println!();
    }

    if !only_manifest.is_empty() {
        println!("Есть в манифесте, но отсутствуют на диске — {}:", only_manifest.len());
        for p in &only_manifest {
            println!("  − {p}");
        }
        println!();
    }

    if !only_disk.is_empty() {
        println!("Есть на диске, но нет в манифесте — {}:", only_disk.len());
        for p in &only_disk {
            println!("  + {p}");
        }
        println!();
    }

    for (p, msize, dsize) in &size_mismatch {
        println!("Размер отличается: {p}");
        println!("    в манифесте: {msize} байт, на диске: {dsize} байт");
    }
    if !size_mismatch.is_empty() {
        println!();
    }

    if !hash_mismatch.is_empty() {
        println!("Хеш не совпадает (при совпадении размера или вместе с ним):");
        for p in &hash_mismatch {
            let (ms, mh) = manifest_map.get(*p).unwrap();
            let (ds, dh) = disk_map.get(*p).unwrap();
            println!("  {p}");
            if ms != ds {
                println!("    размер: манифест {ms} байт, диск {ds} байт");
            }
            println!("    хеш в манифесте: {mh}");
            println!("    хеш на диске:    {dh}");
        }
        println!();
    }

    Ok(false)
}

fn root_base_for_manifest(path: &Path) -> PathBuf {
    if path.is_dir() {
        path.to_path_buf()
    } else {
        path.parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."))
    }
}

fn root_to_manifest_string(path: &Path) -> String {
    let p = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let mut s = p.to_string_lossy().into_owned();
    if let Some(rest) = s.strip_prefix(r"\\?\") {
        s = rest.to_string();
    }
    s.replace('\\', "/")
}

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
