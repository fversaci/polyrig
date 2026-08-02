/**************************************************************************
  Copyright 2026 Francesco Versaci (https://github.com/fversaci/)

  This program is free software: you can redistribute it and/or modify
  it under the terms of the GNU Affero General Public License as published by
  the Free Software Foundation, either version 3 of the License, or
  (at your option) any later version.

  This program is distributed in the hope that it will be useful,
  but WITHOUT ANY WARRANTY; without even the implied warranty of
  MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
  GNU Affero General Public License for more details.

  You should have received a copy of the GNU Affero General Public License
  along with this program.  If not, see <https://www.gnu.org/licenses/>.
**************************************************************************/
use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;

/// Environment variable that overrides the config directory.
pub const CONFIG_DIR_ENV: &str = "POLYRIG_CONFIG_DIR";

/// Bundled template for the talks configuration.
const TALKS_TEMPLATE: &str = include_str!("../conf/talks.toml");
/// Bundled template for the bot defaults, installed as `defaults.toml`.
const DEFAULTS_TEMPLATE: &str = include_str!("../conf/defaults.toml.template");

/// Returns the directory where polyrig keeps its configuration.
///
/// Priority: `$POLYRIG_CONFIG_DIR`, then the platform config dir via the
/// `dirs` crate (e.g. `~/.config/polyrig` on Linux), with a home-dir
/// fallback.
pub fn config_dir() -> PathBuf {
    if let Ok(dir) = std::env::var(CONFIG_DIR_ENV)
        && !dir.trim().is_empty()
    {
        return PathBuf::from(dir);
    }
    dirs::config_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")))
        .join("polyrig")
}

/// Path to the talks configuration file in the config directory.
pub fn talks_config_path() -> PathBuf {
    config_dir().join("talks.toml")
}

/// Path to the bot defaults configuration file in the config directory.
pub fn defaults_config_path() -> PathBuf {
    config_dir().join("defaults.toml")
}

/// Ensures the config directory exists and contains the config files,
/// copying the bundled templates on first run.
///
/// Existing files are never overwritten. A legacy `conf/defaults.toml`
/// present in the working directory is migrated into the config dir so
/// previously configured settings (e.g. the Telegram whitelist) are kept.
pub fn ensure_config() -> Result<()> {
    let dir = config_dir();
    fs::create_dir_all(&dir)
        .with_context(|| format!("Failed to create config directory {}", dir.display()))?;

    let talks_path = dir.join("talks.toml");
    if !talks_path.exists() {
        fs::write(&talks_path, TALKS_TEMPLATE)
            .with_context(|| format!("Failed to write {}", talks_path.display()))?;
    }

    let defaults_path = dir.join("defaults.toml");
    if !defaults_path.exists() {
        let legacy = fs::read_to_string("conf/defaults.toml").ok();
        let contents = legacy.as_deref().unwrap_or(DEFAULTS_TEMPLATE);
        fs::write(&defaults_path, contents)
            .with_context(|| format!("Failed to write {}", defaults_path.display()))?;
    }

    Ok(())
}
