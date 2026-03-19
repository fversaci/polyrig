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
use anyhow::Result;
use rig_core::providers::openrouter;
use rig_core::{agent::Agent, message::Message};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use teloxide::{dispatching::dialogue::InMemStorage, prelude::*};

mod telegram;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MyBotConfig {
    id_whitelist: HashSet<ChatId>,
    #[serde(default = "default_transcription_model")]
    pub transcription_model: String,
    #[serde(default = "default_tts_model")]
    pub tts_model: String,
    #[serde(default = "default_tts_voice")]
    pub tts_voice: String,
}

fn default_transcription_model() -> String {
    openrouter::GPT_4O_MINI_TRANSCRIBE.to_string()
}

fn default_tts_model() -> String {
    openrouter::GPT_4O_MINI_TTS.to_string()
}

fn default_tts_voice() -> String {
    "marin".to_string()
}

#[derive(Clone)]
pub struct MyState {
    my_conf: MyBotConfig,
    agent: Option<Agent<openrouter::CompletionModel>>,
    history: Vec<Message>,
    presuff: (String, String),
    max_hist: Option<usize>,
    voice_reply: bool,
    pub transcription_model: String,
    pub tts_model: String,
    pub tts_voice: String,
}

/// Loads the bot configuration from the TOML file.
fn get_conf() -> MyBotConfig {
    let fname = "conf/defaults.toml";
    let conf_txt = fs::read_to_string(fname)
        .unwrap_or_else(|_| panic!("Cannot find configuration file: {}", fname));
    let my_conf: MyBotConfig = toml::from_str(&conf_txt)
        .unwrap_or_else(|err| panic!("Unable to parse configuration file {}: {}", fname, err));
    my_conf
}

/// Entry point for the Telegram bot binary.
#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .format_timestamp_secs()
        .filter(Some("teloxide::error_handlers"), log::LevelFilter::Warn)
        .filter(
            Some("teloxide::update_listeners::polling"),
            log::LevelFilter::Warn,
        )
        .init();
    log::info!("Starting bot...");
    let bot = Bot::from_env();
    let my_conf = get_conf();
    let transcription_model = my_conf.transcription_model.clone();
    let tts_model = my_conf.tts_model.clone();
    let tts_voice = my_conf.tts_voice.clone();
    log::debug!("{my_conf:?}");
    let my_state = MyState {
        my_conf,
        agent: None,
        history: Vec::new(),
        presuff: ("".to_string(), "".to_string()),
        max_hist: None,
        voice_reply: false,
        transcription_model,
        tts_model,
        tts_voice,
    };
    Dispatcher::builder(bot, telegram::schema(my_state))
        .dependencies(dptree::deps![InMemStorage::<telegram::State>::new()])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;

    Ok(())
}
