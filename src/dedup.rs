//! Поиск дублирующихся файлов в дереве: группировка по размеру, затем по хешу,
//! с опциональным удалением лишних копий.

use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};

use crate::cli::DedupArgs;
use crate::fsutil;
use crate::hash::{self, algo_name};

struct DupGroup {
    size: u64,
    files: Vec<PathBuf>,
}

/// Ищет дубликаты в `args.path` и, если запрошено, удаляет лишние копии.
/// Возвращает `true`, если дубликатов не найдено (дерево «чистое»).
pub fn run(args: &DedupArgs) -> Result<bool> {
    let use_color = std::io::stdout().is_terminal() && !args.no_color;
    let (keep_tag, del_tag) = if use_color {
        ("\x1b[32mKEEP\x1b[0m", "\x1b[31mDELETE\x1b[0m")
    } else {
        ("KEEP", "DELETE")
    };

    let entries = hash::collect_files(&args.path, args.recursive)?;

    let mut by_size: HashMap<u64, Vec<PathBuf>> = HashMap::new();
    for (full_path, _rel_path) in &entries {
        let size = fs::metadata(full_path)?.len();
        if let Some(min_size) = args.min_size {
            if size < min_size {
                continue;
            }
        }
        by_size.entry(size).or_default().push(full_path.clone());
    }

    let mut groups: Vec<DupGroup> = Vec::new();
    for (size, paths) in by_size {
        if paths.len() < 2 {
            continue;
        }
        let mut by_hash: HashMap<String, Vec<PathBuf>> = HashMap::new();
        for path in paths {
            let digest = hash::hash_file(&path, &args.algo)?;
            by_hash.entry(digest).or_default().push(path);
        }
        for mut files in by_hash.into_values() {
            if files.len() < 2 {
                continue;
            }
            files.sort();
            groups.push(DupGroup { size, files });
        }
    }

    groups.sort_by(|a, b| {
        let wasted_a = a.size * (a.files.len() as u64 - 1);
        let wasted_b = b.size * (b.files.len() as u64 - 1);
        wasted_b.cmp(&wasted_a).then_with(|| a.files[0].cmp(&b.files[0]))
    });

    if groups.is_empty() {
        println!("no duplicates found");
        return Ok(true);
    }

    let algo = algo_name(&args.algo);
    let mut total_wasted: u64 = 0;
    let mut total_extra_files: usize = 0;
    for group in &groups {
        let wasted = group.size * (group.files.len() as u64 - 1);
        total_wasted += wasted;
        total_extra_files += group.files.len() - 1;

        println!(
            "duplicate group  ({algo}, {} files x {}, {} reclaimable)",
            group.files.len(),
            fsutil::human_size(group.size),
            fsutil::human_size(wasted)
        );
        for (i, path) in group.files.iter().enumerate() {
            let tag = if i == 0 { keep_tag } else { del_tag };
            println!("  {tag}  {}", path.display());
        }
    }

    println!(
        "\n{} duplicate group(s), {} extra file(s), {} reclaimable",
        groups.len(),
        total_extra_files,
        fsutil::human_size(total_wasted)
    );

    if args.delete {
        delete_duplicates(&args.path, &groups, args.yes, total_extra_files, total_wasted)?;
    }

    Ok(false)
}

fn delete_duplicates(
    root: &Path,
    groups: &[DupGroup],
    yes: bool,
    extra_files: usize,
    wasted: u64,
) -> Result<()> {
    if !yes {
        if io::stdin().is_terminal() {
            print!(
                "\nDelete {extra_files} file(s), freeing {}? [y/N] ",
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
    for group in groups {
        for path in group.files.iter().skip(1) {
            match fs::remove_file(path) {
                Ok(()) => {
                    println!("deleted  {}", path.display());
                    deleted += 1;
                    if let Some(parent) = path.parent() {
                        removed_dirs += fsutil::remove_empty_ancestors(root, parent)?.len();
                    }
                }
                Err(e) => {
                    eprintln!("failed to delete {}: {e}", path.display());
                    failed += 1;
                }
            }
        }
    }
    println!("\ndeleted {deleted} file(s), {removed_dirs} empty dir(s), {failed} failure(s)");

    Ok(())
}
