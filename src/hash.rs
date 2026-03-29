//! Сбор файлов, хеширование и сравнение двух деревьев по хешам.

use crate::cli::Algo;
use anyhow::{Result, anyhow, bail};
use std::collections::HashMap;
use std::fs::{File, read_dir};
use std::io::{self, BufReader, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::cli::HashArgs;

/// Сравнивает `first` и `second`: для каждого файла из первого ищет совпадение по хешу во втором.
/// Возвращает `true`, если все пары совпали по содержимому (хешу).
pub fn run_compare(args: &HashArgs) -> Result<bool> {
    let use_color = std::io::stdout().is_terminal() && !args.no_color;
    let (ok, fail) = if use_color {
        ("\x1b[32m✔\x1b[0m", "\x1b[31m✖\x1b[0m")
    } else {
        ("OK", "DIFF")
    };

    let files_first = collect_files_for_comparison(&args.first)?;
    let files_second = collect_files_for_comparison(&args.second)?;

    if files_first.len() != files_second.len() {
        println!(
            "{fail} different number of files: {} vs {}",
            files_first.len(),
            files_second.len()
        );
        return Ok(false);
    }

    let mut all_identical = true;

    let mut second_by_hash: HashMap<String, Vec<&PathBuf>> = HashMap::new();
    for (path2, rel_path2) in &files_second {
        let hash2 = hash_file(path2, &args.algo)?;
        second_by_hash.entry(hash2).or_default().push(rel_path2);
    }

    for (path1, rel_path1) in &files_first {
        let hash1 = hash_file(path1, &args.algo)?;
        let matched = if let Some(rel_list) = second_by_hash.get_mut(&hash1) {
            if let Some(rel_path2) = rel_list.pop() {
                let same_path = rel_path1 == rel_path2;
                if same_path {
                    println!("{ok}  identical  {}  ({})", rel_path1.display(), algo_name(&args.algo));
                } else {
                    println!(
                        "{ok}  identical  {}  {}  ({})",
                        rel_path1.display(),
                        rel_path2.display(),
                        algo_name(&args.algo)
                    );
                }
                if rel_list.is_empty() {
                    second_by_hash.remove(&hash1);
                }
                true
            } else {
                false
            }
        } else {
            false
        };

        if !matched {
            println!(
                "{fail} different  {}  ({})",
                rel_path1.display(),
                algo_name(&args.algo)
            );
            all_identical = false;
        }
    }

    Ok(all_identical)
}

/// Список файлов относительно корня (файл или дерево каталога), порядок — как при обходе, затем сортировка в вызывающем коде при необходимости.
pub fn collect_files_for_comparison(path: &Path) -> Result<Vec<(PathBuf, PathBuf)>> {
    if path.is_file() {
        let rel_path = path
            .file_name()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        Ok(vec![(path.to_path_buf(), rel_path)])
    } else if path.is_dir() {
        let mut entries: Vec<(PathBuf, PathBuf)> = Vec::new();
        collect_files_recursive(path, path, &mut entries)?;
        entries.sort_by_key(|(_, rel_path)| rel_path.clone());
        Ok(entries)
    } else {
        bail!("Path is neither a file nor a directory: {}", path.display())
    }
}

/// Строковое имя алгоритма для вывода и JSON.
pub fn algo_name(algo: &Algo) -> &'static str {
    match algo {
        Algo::Sha256 => "sha256",
        Algo::Sha384 => "sha384",
        Algo::Sha512 => "sha512",
        Algo::Sha3_256 => "sha3-256",
        Algo::Sha3_384 => "sha3-384",
        Algo::Sha3_512 => "sha3-512",
        Algo::Blake3 => "blake3",
    }
}

/// Распознаёт алгоритм из поля `algorithm` в манифесте.
pub fn algo_from_manifest_name(s: &str) -> Result<Algo> {
    match s.trim() {
        "sha256" => Ok(Algo::Sha256),
        "sha384" => Ok(Algo::Sha384),
        "sha512" => Ok(Algo::Sha512),
        "sha3-256" => Ok(Algo::Sha3_256),
        "sha3-384" => Ok(Algo::Sha3_384),
        "sha3-512" => Ok(Algo::Sha3_512),
        "blake3" => Ok(Algo::Blake3),
        other => bail!("неизвестный алгоритм в манифесте: {other:?}"),
    }
}

/// Хеш содержимого файла выбранным алгоритмом.
pub fn hash_file(path: &Path, algo: &Algo) -> Result<String> {
    let p = PathBuf::from(path);
    match algo {
        Algo::Blake3 => hash_blake3(&p),
        Algo::Sha256 => hash_sha256(&p),
        Algo::Sha384 => hash_sha384(&p),
        Algo::Sha512 => hash_sha512(&p),
        Algo::Sha3_256 => hash_sha3_256(&p),
        Algo::Sha3_384 => hash_sha3_384(&p),
        Algo::Sha3_512 => hash_sha3_512(&p),
    }
}

fn collect_files_recursive(root: &Path, current: &Path, entries: &mut Vec<(PathBuf, PathBuf)>) -> Result<()> {
    for entry in read_dir(current)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_file() {
            let rel_path = path
                .strip_prefix(root)
                .map_err(|_| anyhow!("Failed to get relative path"))?
                .to_path_buf();
            entries.push((path, rel_path));
        } else if path.is_dir() {
            collect_files_recursive(root, &path, entries)?;
        }
    }
    Ok(())
}

struct Spinner {
    frames: [&'static str; 4],
    idx: usize,
    last: Instant,
    interval: Duration,
    enabled: bool,
    colored: bool,
}

impl Spinner {
    fn new(interval: Duration, colored: bool) -> Self {
        let enabled = io::stderr().is_terminal();
        Self {
            frames: ["|", "/", "-", "\\"],
            idx: 0,
            last: Instant::now(),
            interval,
            enabled,
            colored,
        }
    }

    fn tick(&mut self, msg: &str) {
        if !self.enabled || self.last.elapsed() < self.interval {
            return;
        }
        let frame = self.frames[self.idx];
        self.idx = (self.idx + 1) % self.frames.len();
        if self.colored {
            eprint!("\r\x1b[32m{}\x1b[0m {}", frame, msg);
        } else {
            eprint!("\r{} {}", frame, msg);
        }
        let _ = io::stderr().flush();
        self.last = Instant::now();
    }

    fn finish(&mut self) {
        if !self.enabled {
            return;
        }
        eprint!("\r\x1b[K");
        let _ = io::stderr().flush();
    }
}

fn hash_blake3(path: &PathBuf) -> Result<String> {
    let mut r = BufReader::new(File::open(path)?);
    let mut hasher = blake3::Hasher::new();
    let mut buf = [0u8; 64 * 1024];
    let mut sp = Spinner::new(Duration::from_millis(80), true);
    loop {
        let n = std::io::Read::read(&mut r, &mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        sp.tick("");
    }
    sp.finish();
    Ok(hasher.finalize().to_hex().to_string())
}

fn hash_sha256(path: &PathBuf) -> Result<String> {
    use sha2::Sha256;
    stream_hash::<Sha256>(path)
}

fn hash_sha384(path: &PathBuf) -> Result<String> {
    use sha2::Sha384;
    stream_hash::<Sha384>(path)
}

fn hash_sha512(path: &PathBuf) -> Result<String> {
    use sha2::Sha512;
    stream_hash::<Sha512>(path)
}

fn hash_sha3_256(path: &PathBuf) -> Result<String> {
    use sha3::Sha3_256;
    stream_hash::<Sha3_256>(path)
}

fn hash_sha3_384(path: &PathBuf) -> Result<String> {
    use sha3::Sha3_384;
    stream_hash::<Sha3_384>(path)
}

fn hash_sha3_512(path: &PathBuf) -> Result<String> {
    use sha3::Sha3_512;
    stream_hash::<Sha3_512>(path)
}

fn stream_hash<D: digest::Digest + Default>(path: &PathBuf) -> Result<String> {
    let mut r = BufReader::new(File::open(path)?);
    let mut h = D::default();
    let mut buf = [0u8; 64 * 1024];
    let mut sp = Spinner::new(Duration::from_millis(80), true);
    loop {
        let n = std::io::Read::read(&mut r, &mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
        sp.tick("");
    }
    sp.finish();
    Ok(hex::encode(h.finalize()))
}
