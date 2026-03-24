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
    /// Создать JSON-манифест набора файлов в каталоге.
    Manifest(ManifestArgs),
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

/// Аргументы команды `manifest`.
#[derive(Parser, Debug)]
pub struct ManifestArgs {
    /// Корневая директория или один файл для описания.
    #[arg(
        short,
        long,
        help = "Путь к корневой директории или одном файлу в формате /dir1/targed-dir"
    )]
    pub path: PathBuf,

    /// Путь к выходному JSON-файлу (UTF-8).
    #[arg(
        short,
        long,
        help = "Путь к выходному JSON-файлу (UTF-8) в формате /dir1/target-dir/manifest.json"
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

/// Алгоритмы хеширования для команды `hash`.
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
