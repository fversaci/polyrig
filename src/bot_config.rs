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
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use crate::config;

/// Voice used as a fallback when the configured TTS model has no
/// discoverable voices and `tts_voice` is not set in the configuration.
pub const DEFAULT_TTS_VOICE: &str = "en-US-Harper:MAI-Voice-2.1";

/// Bot defaults loaded from `defaults.toml` in the config directory.
///
/// `tts_voice` is optional: when absent, the bot picks a random voice
/// from the TTS model's supported voices for each new conversation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BotConfig {
    pub id_whitelist: HashSet<teloxide::types::ChatId>,
    #[serde(default = "default_transcription_model")]
    pub transcription_model: String,
    #[serde(default = "default_tts_model")]
    pub tts_model: String,
    #[serde(default)]
    pub tts_voice: Option<String>,
    #[serde(default = "default_tts_format")]
    pub tts_format: String,
    #[serde(default = "default_max_tts_tokens")]
    pub max_tts_tokens: u64,
}

fn default_transcription_model() -> String {
    "nvidia/nemotron-3.5-asr-streaming-multilingual-0.6b".to_string()
}

pub fn default_tts_model() -> String {
    "microsoft/mai-voice-2.1-flash".to_string()
}

fn default_tts_format() -> String {
    "mp3".to_string()
}

fn default_max_tts_tokens() -> u64 {
    10000
}

impl Default for BotConfig {
    fn default() -> Self {
        toml::from_str("").expect("BotConfig default is always valid")
    }
}

/// Loads the bot configuration from `defaults.toml` in the config directory.
pub fn get_conf() -> anyhow::Result<BotConfig> {
    config::ensure_config()?;
    let fname = config::defaults_config_path();
    let conf_txt = std::fs::read_to_string(&fname)
        .map_err(|e| anyhow::anyhow!("Cannot find configuration file {}: {e}", fname.display()))?;
    toml::from_str(&conf_txt).map_err(|e| {
        anyhow::anyhow!(
            "Unable to parse configuration file {}: {e}",
            fname.display()
        )
    })
}
