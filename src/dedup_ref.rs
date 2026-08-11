//! Дедупликация `target` относительно `reference`: удаляет из `target` файлы, чьё
//! содержимое (хеш) уже присутствует в `reference`. `reference` никогда не изменяется.

use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};

use crate::cli::DedupRefArgs;
use crate::fsutil;
use crate::hash::{self, algo_name};

struct DupMatch {
    target: PathBuf,
    reference: PathBuf,
    size: u64,
}

/// Ищет в `args.target` файлы, дублирующие содержимое `args.reference`, и, если запрошено,
/// удаляет их из `target`. Возвращает `true`, если дубликатов не найдено.
pub fn run(args: &DedupRefArgs) -> Result<bool> {
    let use_color = std::io::stdout().is_terminal() && !args.no_color;
    let del_tag = if use_color { "\x1b[31mDELETE\x1b[0m" } else { "DELETE" };

    let target_entries = hash::collect_files(&args.target, args.recursive)?;
    let reference_entries = hash::collect_files(&args.reference, args.recursive)?;

    let mut target_by_size: HashMap<u64, Vec<PathBuf>> = HashMap::new();
    for (full_path, _rel_path) in &target_entries {
        let size = fs::metadata(full_path)?.len();
        if let Some(min_size) = args.min_size {
            if size < min_size {
                continue;
            }
        }
        target_by_size.entry(size).or_default().push(full_path.clone());
    }

    let mut reference_by_size: HashMap<u64, Vec<(PathBuf, PathBuf)>> = HashMap::new();
    for (full_path, rel_path) in &reference_entries {
        let size = fs::metadata(full_path)?.len();
        if !target_by_size.contains_key(&size) {
            continue;
        }
        reference_by_size
            .entry(size)
            .or_default()
            .push((full_path.clone(), rel_path.clone()));
    }

    let mut matches: Vec<DupMatch> = Vec::new();
    for (size, target_files) in &target_by_size {
        let Some(reference_files) = reference_by_size.get(size) else {
            continue;
        };

        let mut reference_hashes: HashMap<String, PathBuf> = HashMap::new();
        for (full_path, rel_path) in reference_files {
            let digest = hash::hash_file(full_path, &args.algo)?;
            reference_hashes.entry(digest).or_insert_with(|| rel_path.clone());
        }

        for target_full in target_files {
            let digest = hash::hash_file(target_full, &args.algo)?;
            if let Some(reference_rel) = reference_hashes.get(&digest) {
                matches.push(DupMatch {
                    target: target_full.clone(),
                    reference: reference_rel.clone(),
                    size: *size,
                });
            }
        }
    }

    if matches.is_empty() {
        println!("no duplicates found");
        return Ok(true);
    }

    matches.sort_by(|a, b| a.target.cmp(&b.target));

    let algo = algo_name(&args.algo);
    let total_wasted: u64 = matches.iter().map(|m| m.size).sum();
    for m in &matches {
        println!(
            "{del_tag}  {}  (dup of reference/{}, {})",
            m.target.display(),
            m.reference.display(),
            algo
        );
    }

    println!(
        "\n{} duplicate file(s) in target, {} reclaimable",
        matches.len(),
        fsutil::human_size(total_wasted)
    );

    if args.delete {
        delete_matches(&args.target, &matches, args.yes, total_wasted)?;
    }

    Ok(false)
}

fn delete_matches(target_root: &Path, matches: &[DupMatch], yes: bool, wasted: u64) -> Result<()> {
    if !yes {
        if io::stdin().is_terminal() {
            print!(
                "\nDelete {} file(s) from target, freeing {}? [y/N] ",
                matches.len(),
                fsutil::human_size(wasted)
            );
            io::stdout().flush()?;
            let mut answer = String::new();
            io::stdin().read_line(&mut answer)?;
            if !matches!(answer.trim().to_lowercase().as_str(), "y" | "yes") {
                println!("aborted, nothing deleted");
                return Ok(());
            }
        } else {
            println!("\nrefusing to delete without --yes in non-interactive mode");
            return Ok(());
        }
    }

    let mut deleted = 0usize;
    let mut failed = 0usize;
    let mut removed_dirs = 0usize;
    for m in matches {
        match fs::remove_file(&m.target) {
            Ok(()) => {
                println!("deleted  {}", m.target.display());
                deleted += 1;
                if let Some(parent) = m.target.parent() {
                    removed_dirs += fsutil::remove_empty_ancestors(target_root, parent)?.len();
                }
            }
            Err(e) => {
                eprintln!("failed to delete {}: {e}", m.target.display());
                failed += 1;
            }
        }
    }
    println!("\ndeleted {deleted} file(s), {removed_dirs} empty dir(s), {failed} failure(s)");

    Ok(())
}
