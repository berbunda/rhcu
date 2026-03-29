//! Точка входа: модули, разбор CLI, диспетчеризация команд и коды выхода.

mod cli;
mod error;
mod hash;
mod manifest;

use clap::Parser;

use crate::cli::{Cli, Commands, ManifestSubcommand};

fn main() {
    match run_app() {
        Ok(code) => std::process::exit(code),
        Err(e) => {
            error::report(&e);
            std::process::exit(1);
        }
    }
}

/// Выполняет выбранную команду: `0` — успех; `1` — есть отличия при `hash` (содержимое не сошлось).
fn run_app() -> anyhow::Result<i32> {
    let cli = Cli::parse();
    match &cli.command {
        Commands::Hash(args) => {
            let identical = hash::run_compare(args)?;
            Ok(if identical { 0 } else { 1 })
        }
        Commands::Manifest(cmd) => match &cmd.sub {
            ManifestSubcommand::Create(args) => {
                manifest::create(args)?;
                Ok(0)
            }
            ManifestSubcommand::Verify(args) => {
                let ok = manifest::verify(args)?;
                Ok(if ok { 0 } else { 1 })
            }
            ManifestSubcommand::Update(args) => {
                manifest::update(args)?;
                Ok(0)
            }
        },
    }
}
