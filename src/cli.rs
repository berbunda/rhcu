//! Аргументы командной строки (clap): корневая структура, подкоманды и их параметры.

use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

/// Корневой парсер CLI.
#[derive(Parser, Debug)]
#[command(name = "rhcu", version, about)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

/// Подкоманды приложения.
#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Сравнить два дерева файлов по хешам (порядок имён не важен).
    Hash(HashArgs),
    /// Манифест: создать JSON или сверить каталог с manifest.json.
    Manifest(ManifestCommand),
    /// Операции над деревом файлов: поиск дубликатов и (в будущем) слияние директорий.
    Fs(FsCommand),
}

/// Аргументы команды `hash`.
#[derive(Parser, Debug)]
pub struct HashArgs {
    #[arg(
        value_enum,
        short,
        long,
        default_value_t = Algo::Blake3,
        help = "Используемый алгоритм хеширования"
    )]
    pub algo: Algo,

    #[arg(short, long, help = "Отключить ANSI цвета")]
    pub no_color: bool,

    #[arg(short, long, required = true, help = "Первая директория для сравнения")]
    pub first: PathBuf,

    #[arg(short, long, required = true, help = "Вторая директория для сравнения")]
    pub second: PathBuf,
}

/// Подкоманды `manifest`: `create` / `verify` / `update`.
#[derive(Parser, Debug)]
pub struct ManifestCommand {
    #[command(subcommand)]
    pub sub: ManifestSubcommand,
}

#[derive(Subcommand, Debug)]
pub enum ManifestSubcommand {
    /// Записать JSON-манифест набора файлов.
    Create(ManifestCreateArgs),
    /// Сверить содержимое каталога с manifest.json (размер и хеш каждого файла).
    Verify(ManifestVerifyArgs),
    /// Пересчитать манифест по текущему дереву: `revision + 1`, новый `timestamp`.
    Update(ManifestUpdateArgs),
}

/// Аргументы `manifest create`.
#[derive(Parser, Debug)]
pub struct ManifestCreateArgs {
    #[arg(
        short,
        long,
        help = "Путь к корневой директории или одному файлу (/target-dir или C:/target-dir)"
    )]
    pub path: PathBuf,

    #[arg(
        short,
        long,
        help = "Путь к выходному JSON (UTF-8), (/target-dir/manifest.json или C:/target-dir/manifest.json)"
    )]
    pub output: PathBuf,

    #[arg(
        value_enum,
        short,
        long,
        default_value_t = Algo::Blake3,
        help = "Алгоритм хеширования для поля manifest \"algorithm\""
    )]
    pub algo: Algo,
}

/// Аргументы `manifest verify`.
#[derive(Parser, Debug)]
pub struct ManifestVerifyArgs {
    /// Каталог (или корень, как при create), содержимое которого сверяется с манифестом.
    #[arg(
        short,
        long,
        help = "Каталог для проверки (ожидается manifest.json внутри, если не задан --manifest)"
    )]
    pub path: PathBuf,

    /// Путь к JSON манифесту; по умолчанию `<path>/manifest.json`.
    #[arg(short, long)]
    pub manifest: Option<PathBuf>,
}

/// Аргументы `manifest update`.
#[derive(Parser, Debug)]
pub struct ManifestUpdateArgs {
    /// Каталог для обхода (как при `create`); манифест перезаписывается.
    #[arg(
        short,
        long,
        help = "Корень дерева файлов; по умолчанию manifest.json — <path>/manifest.json"
    )]
    pub path: PathBuf,

    /// Путь к JSON манифесту; по умолчанию `<path>/manifest.json`.
    #[arg(short, long)]
    pub manifest: Option<PathBuf>,

    /// Заменить алгоритм хеширования (по умолчанию берётся из существующего манифеста).
    #[arg(value_enum, short, long)]
    pub algo: Option<Algo>,
}

/// Подкоманды `fs`: `dedup` (в будущем — `merge`).
#[derive(Parser, Debug)]
pub struct FsCommand {
    #[command(subcommand)]
    pub sub: FsSubcommand,
}

#[derive(Subcommand, Debug)]
pub enum FsSubcommand {
    /// Найти и (опционально) удалить дублирующиеся файлы в дереве.
    Dedup(DedupArgs),
    /// Удалить в target файлы, дублирующие содержимое reference (reference не изменяется).
    DedupRef(DedupRefArgs),
}

/// Аргументы `fs dedup`.
#[derive(Parser, Debug)]
pub struct DedupArgs {
    #[arg(short, long, help = "Корневая директория или файл для поиска дубликатов")]
    pub path: PathBuf,

    #[arg(
        value_enum,
        short,
        long,
        default_value_t = Algo::Blake3,
        help = "Используемый алгоритм хеширования"
    )]
    pub algo: Algo,

    #[arg(short, long, help = "Отключить ANSI цвета")]
    pub no_color: bool,

    #[arg(short, long, help = "Удалить дубликаты (оставив по одному файлу в каждой группе)")]
    pub delete: bool,

    #[arg(short, long, help = "Не спрашивать подтверждения перед удалением")]
    pub yes: bool,

    #[arg(long, help = "Игнорировать файлы меньше заданного размера в байтах")]
    pub min_size: Option<u64>,

    #[arg(
        long,
        help = "Обходить вложенные поддиректории (по умолчанию — только верхний уровень)"
    )]
    pub recursive: bool,
}

/// Аргументы `fs dedup-ref`.
#[derive(Parser, Debug)]
pub struct DedupRefArgs {
    #[arg(
        short,
        long,
        help = "Целевая директория: отсюда удаляются файлы, дублирующие содержимое reference"
    )]
    pub target: PathBuf,

    #[arg(
        short,
        long,
        help = "Эталонная директория для сравнения; никогда не изменяется"
    )]
    pub reference: PathBuf,

    #[arg(
        value_enum,
        short,
        long,
        default_value_t = Algo::Blake3,
        help = "Используемый алгоритм хеширования"
    )]
    pub algo: Algo,

    #[arg(short, long, help = "Отключить ANSI цвета")]
    pub no_color: bool,

    #[arg(short, long, help = "Удалить из target файлы, дублирующие reference")]
    pub delete: bool,

    #[arg(short, long, help = "Не спрашивать подтверждения перед удалением")]
    pub yes: bool,

    #[arg(long, help = "Игнорировать файлы меньше заданного размера в байтах")]
    pub min_size: Option<u64>,

    #[arg(
        long,
        help = "Обходить вложенные поддиректории в target и reference (по умолчанию — только верхний уровень)"
    )]
    pub recursive: bool,
}

/// Алгоритмы хеширования для команды `hash` и `manifest create`.
#[derive(ValueEnum, Clone, Debug, Default)]
pub enum Algo {
    Sha256,
    Sha384,
    Sha512,
    Sha3_256,
    Sha3_384,
    Sha3_512,
    #[default]
    Blake3,
}
