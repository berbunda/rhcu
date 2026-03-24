//! Манифест: логика будет добавлена позже.

use crate::cli::ManifestArgs;

/// Заглушка до реализации команды `manifest`.
pub fn run(args: &ManifestArgs) -> anyhow::Result<()> {
    anyhow::bail!(
        "команда manifest пока не реализована{}",
        args.path
            .as_ref()
            .map(|p| format!(" (path: {})", p.display()))
            .unwrap_or_default()
    );
}
