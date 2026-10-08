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
use polyrig::bot_config::{self, BotConfig};
use rig_core::providers::openrouter;
use rig_core::{agent::Agent, message::Message};
use teloxide::{dispatching::dialogue::InMemStorage, prelude::*};

mod telegram;

#[derive(Clone)]
pub struct MyState {
    my_conf: BotConfig,
    agent: Option<Agent<openrouter::CompletionModel>>,
    history: Vec<Message>,
    presuff: (String, String),
    max_hist: Option<usize>,
    voice_reply: bool,
    pub transcription_model: String,
    pub tts_model: String,
    pub tts_voice: String,
    pub tts_format: String,
    pub max_tts_tokens: u64,
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
    let my_conf: BotConfig =
        bot_config::get_conf().unwrap_or_else(|err| panic!("Failed to load configuration: {err}"));
    let bot = Bot::from_env();
    let transcription_model = my_conf.transcription_model.clone();
    let tts_model = my_conf.tts_model.clone();
    let tts_voice = my_conf.tts_voice.clone().unwrap_or_default();
    let tts_format = my_conf.tts_format.clone();
    let max_tts_tokens = my_conf.max_tts_tokens;
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
        tts_format,
        max_tts_tokens,
    };
    Dispatcher::builder(bot, telegram::schema(my_state))
        .dependencies(dptree::deps![InMemStorage::<telegram::State>::new()])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;

    Ok(())
}
