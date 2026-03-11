//Импорт зависимостей
use anyhow::{Result, bail};
use clap::{Parser, ValueEnum};
use std::fs::{File, read_dir};
use std::io::{self, BufReader, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

//Определение перечисления для выбора алгоритма хеширования
#[derive(ValueEnum, Clone, Debug)]
enum Algo {
    Sha256,
    Sha384,
    Sha512,
    Sha3_256,
    Sha3_384,
    Sha3_512,
    Blake3
}

//Определение структуры для аргументов командной строки
#[derive(Parser, Debug)]
struct Args {
//Используемый алгоритм хеширования
    #[arg(value_enum, long, default_value_t = Algo::Blake3)]
    algo: Algo,

// Отключить ANSI цвета
    #[arg(long)]
    no_color: bool,

// Первая директория
    first: PathBuf,

// Вторая директория
    second: PathBuf,
}

//Главная функция программы
fn main() -> Result<()> {
    let args = Args::parse();

    let use_color = std::io::stdout().is_terminal() && !args.no_color;
    let (ok, fail) = if use_color {
        ("\x1b[32m✔\x1b[0m", "\x1b[31m✖\x1b[0m")
    } else { ("OK", "DIFF") };

    // Собираем списки файлов для сравнения
    let files_first = collect_files_for_comparison(&args.first)?;
    let files_second = collect_files_for_comparison(&args.second)?;

    // Проверяем, что количество файлов совпадает
    if files_first.len() != files_second.len() {
        println!("{fail} different number of files: {} vs {}", files_first.len(), files_second.len());
        std::process::exit(1);
    }

    let mut all_identical = true;

    // Сравниваем файлы по отдельности
    for ((path1, rel_path1), (path2, rel_path2)) in files_first.iter().zip(files_second.iter()) {
        let hash1 = hash_file(path1, &args.algo)?;
        let hash2 = hash_file(path2, &args.algo)?;
        
        let same_hash = hash1 == hash2;
        let same_path = rel_path1 == rel_path2;
        if same_hash {
            if same_path {
                println!("{ok}  identical  {}  ({})", rel_path1.display(), algo_name(&args.algo));
            } else { 
                println!("{ok}  identical  {}  {}  ({})", rel_path1.display(), rel_path2.display(), algo_name(&args.algo));
            }
        } else {
            if same_path {
                println!("{fail} different  {}  ({})", rel_path1.display(), algo_name(&args.algo));
                all_identical = false;
            } else {
                print!("{fail} different {}  {}  ({})", rel_path1.display(), rel_path2.display(), algo_name(&args.algo));
                all_identical = false;
            }
        }
    }

        if all_identical {
            std::process::exit(0);
        } else {
            std::process::exit(1);
        }
    }


//Функция для сбора файлов для сравнения (поддерживает как файлы, так и директории)
fn collect_files_for_comparison(path: &Path) -> Result<Vec<(PathBuf, PathBuf)>> {
    if path.is_file() {
        // Для файла возвращаем его с пустым относительным путём (или именем файла)
        let rel_path = path.file_name()
            .map(|n| PathBuf::from(n))
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

//Функция определения хэш-функции на основе выбранного алгоритма
fn algo_name(algo_name: &Algo) -> &'static str {
    match algo_name {
        Algo::Sha256 => "sha256",
        Algo::Sha384 => "sha384",
        Algo::Sha512 => "sha512",
        Algo::Sha3_256 => "sha3-256",
        Algo::Sha3_384 => "sha3-384",
        Algo::Sha3_512 => "sha3-512",
        Algo::Blake3 => "blake3",
    }
}

//Функция для хеширования файла или директории с использованием выбранного алгоритма
/*
#[allow(dead_code)]
fn hash_path(path: &Path, algo: &Algo) -> Result<String> {
    if path.is_file() {
        hash_file(path, algo)
    } else if path.is_dir() {
        hash_directory(path, algo)
    } else {
        bail!("Path is neither a file nor a directory: {}", path.display())
    }
}
*/

//Функция для хеширования файла с использованием выбранного алгоритма
fn hash_file(path: &Path, algo: &Algo) -> Result<String> {
    match algo {
        Algo::Blake3   => hash_blake3(&PathBuf::from(path)),
        Algo::Sha256   => hash_sha256(&PathBuf::from(path)),
        Algo::Sha384   => hash_sha384(&PathBuf::from(path)),
        Algo::Sha512   => hash_sha512(&PathBuf::from(path)),
        Algo::Sha3_256 => hash_sha3_256(&PathBuf::from(path)),
        Algo::Sha3_384 => hash_sha3_384(&PathBuf::from(path)),
        Algo::Sha3_512 => hash_sha3_512(&PathBuf::from(path)),
    }
}

//Функция для рекурсивного хеширования директории
/*
#[allow(dead_code)]
fn hash_directory(path: &Path, algo: &Algo) -> Result<String> {
    let mut entries: Vec<(PathBuf, PathBuf)> = Vec::new(); // (полный путь, относительный путь)
    collect_files_recursive(path, path, &mut entries)?;
    
    // Сортируем по относительным путям для детерминированного порядка
    entries.sort_by_key(|(_, rel_path)| rel_path.clone());
    
    match algo {
        Algo::Blake3 => hash_directory_blake3(&entries),
        Algo::Sha256 => hash_directory_stream::<sha2::Sha256>(&entries),
        Algo::Sha384 => hash_directory_stream::<sha2::Sha384>(&entries),
        Algo::Sha512 => hash_directory_stream::<sha2::Sha512>(&entries),
        Algo::Sha3_256 => hash_directory_stream::<sha3::Sha3_256>(&entries),
        Algo::Sha3_384 => hash_directory_stream::<sha3::Sha3_384>(&entries),
        Algo::Sha3_512 => hash_directory_stream::<sha3::Sha3_512>(&entries),
    }
}
*/

//Рекурсивный сбор всех файлов из директории
fn collect_files_recursive(root: &Path, current: &Path, entries: &mut Vec<(PathBuf, PathBuf)>) -> Result<()> {
    for entry in read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        
        if path.is_file() {
            let rel_path = path.strip_prefix(root)
                .map_err(|_| anyhow::anyhow!("Failed to get relative path"))?
                .to_path_buf();
            entries.push((path, rel_path));
        } else if path.is_dir() {
            collect_files_recursive(root, &path, entries)?;
        }
    }
    Ok(())
}

//Хеширование директории с использованием Blake3
/*
#[allow(dead_code)]
fn hash_directory_blake3(entries: &[(PathBuf, PathBuf)]) -> Result<String> {
    let mut hasher = blake3::Hasher::new();
    let mut sp = Spinner::new(Duration::from_millis(80), true);
    
    for (full_path, rel_path) in entries {
        sp.tick(&format!("Processing: {}", rel_path.display()));
        
        // Добавляем относительный путь файла в хеш для детерминированности
        hasher.update(rel_path.to_string_lossy().as_bytes());
        hasher.update(b"\0");
        
        // Добавляем содержимое файла
        let mut r = BufReader::new(File::open(full_path)?);
        let mut buf = [0u8; 64 * 1024];
        loop {
            let n = std::io::Read::read(&mut r, &mut buf)?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
        }
    }
    
    sp.finish();
    Ok(hasher.finalize().to_hex().to_string())
}
*/

//Хеширование директории с использованием stream hash
/*
#[allow(dead_code)]
fn hash_directory_stream<D: digest::Digest + Default>(entries: &[(PathBuf, PathBuf)]) -> Result<String> {
    let mut hasher = D::default();
    let mut sp = Spinner::new(Duration::from_millis(80), true);
    
    for (full_path, rel_path) in entries {
        sp.tick(&format!("Processing: {}", rel_path.display()));
        
        // Добавляем относительный путь файла в хеш для детерминированности
        hasher.update(rel_path.to_string_lossy().as_bytes());
        hasher.update(b"\0");
        
        // Добавляем содержимое файла
        let mut r = BufReader::new(File::open(full_path)?);
        let mut buf = [0u8; 64 * 1024];
        loop {
            let n = std::io::Read::read(&mut r, &mut buf)?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
        }
    }
    
    sp.finish();
    Ok(hex::encode(hasher.finalize()))
}
*/

//Определение структуры для анимированного индикатора загрузки
struct Spinner {
    frames: [&'static str; 4],
    idx: usize,
    last: Instant,
    interval: Duration,
    enabled: bool,
    colored: bool,
}

//Определение методов для структуры Spinner
impl Spinner {
    fn new(interval: Duration, colored: bool) -> Self {
        let enabled = io::stderr().is_terminal(); // не TTY? молчим
        Self {
            frames: ["|", "/", "-", "\\"],
            idx: 0,
            last: Instant::now(),
            interval,
            enabled,
            colored,
        }
    }

// Функция обновления индикатора загрузки
    fn tick(&mut self, msg: &str) {
        if !self.enabled || self.last.elapsed() < self.interval {
            return;
        }
        let frame = self.frames[self.idx];
        self.idx = (self.idx + 1) % self.frames.len();
        // \r — в начало строки; \x1b[K — очистить до конца строки
        if self.colored {
            eprint!("\r\x1b[32m{}\x1b[0m {}", frame, msg);
        } else {
            eprint!("\r{} {}", frame, msg);
        }
        let _ = io::stderr().flush();
        self.last = Instant::now();
    }
    
// Функция завершения индикатора загрузки
    fn finish(&mut self) {
        if !self.enabled {
            return;
        }
        eprint!("\r\x1b[K"); // стереть строку
        let _ = io::stderr().flush();
    }
}

//Функция blake3  для хеширования файлов
fn hash_blake3(path: &PathBuf) -> Result<String> {
    let mut r = BufReader::new(File::open(path)?);
    let mut hasher = blake3::Hasher::new();
    let mut buf = [0u8; 64 * 1024];
    let mut sp = Spinner::new(Duration::from_millis(80), /*colored=*/ true);
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

//Функция sha256 для хеширования файлов
fn hash_sha256(path: &PathBuf) -> Result<String> {
    use sha2::Sha256;
    stream_hash::<Sha256>(path)
}

//Функция sha384 для хеширования файлов
fn hash_sha384(path: &PathBuf) -> Result<String> {
    use sha2::Sha384;
    stream_hash::<Sha384>(path)
}

//Функция sha512 для хеширования файлов
fn hash_sha512(path: &PathBuf) -> Result<String> {
    use sha2::Sha512;
    stream_hash::<Sha512>(path)
}

//Функция sha3-256 для хеширования файлов
fn hash_sha3_256(path: &PathBuf) -> Result<String> {
    use sha3::Sha3_256;
    stream_hash::<Sha3_256>(path)
}

//Функция sha3-384 для хеширования файлов
fn hash_sha3_384(path: &PathBuf) -> Result<String> {
    use sha3::Sha3_384;
    stream_hash::<Sha3_384>(path)
}

//Функция sha3-512 для хеширования файлов
fn hash_sha3_512(path: &PathBuf) -> Result<String> {
    use sha3::Sha3_512;
    stream_hash::<Sha3_512>(path)
}

//Универсальная функция для хеширования файлов с использованием заданного алгоритма
fn stream_hash<D: digest::Digest + Default>(path: &PathBuf) -> Result<String> {
    let mut r = BufReader::new(File::open(path)?);
    let mut h = D::default();
    let mut buf = [0u8; 64 * 1024];
    let mut sp = Spinner::new(Duration::from_millis(80), /*colored=*/ true);
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
