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
pub mod lang_practice;

use crate::config;
use anyhow::{Error, Result};
use async_stream::stream;
use clap::Subcommand;
use lang_practice::{Lang, LangLevel};
use rig_core::agent::Agent;
use rig_core::agent::MultiTurnStreamItem;
use rig_core::agent::StreamingResult;
use rig_core::client::CompletionClient;
use rig_core::client::ProviderClient;
use rig_core::message::Message;
use rig_core::providers::openrouter;
use rig_core::providers::openrouter::streaming::StreamingCompletionResponse;
use rig_core::streaming::StreamedAssistantContent;
use rig_core::streaming::StreamingChat;
use serde::Deserialize;
use serde_json;
use std::collections::HashMap;
use strum_macros::{Display, EnumIter, EnumString};
use tokio_stream::Stream;
use tokio_stream::StreamExt;

#[derive(Debug, Deserialize)]
struct TalkConfig {
    system_prompt: String,
    prefix: String,
    suffix: String,
    max_hist: Option<usize>,
    first_msg: Option<String>,
    generate_response: bool,
    model: String,
    temperature: Option<f64>,
    additional_params: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct TalksConfig {
    talks: HashMap<String, TalkConfig>,
}

pub struct Conversation {
    pub agent: Agent<openrouter::CompletionModel>,
    pub first_msg: Option<String>,
    pub presuff: (String, String),
    pub max_hist: Option<usize>, // Maximum history length for conversation
    pub history: Vec<Message>,   // Message history
    pub pending_user_msg: Option<Message>, // User message waiting for assistant response
}

/// Converts a streaming completion result into a stream of string chunks.
pub fn stream_messages<M>(mut stream: StreamingResult<M>) -> impl Stream<Item = Result<String>> {
    stream! {
        while let Some(content) = stream.next().await {
            match content {
                Ok(MultiTurnStreamItem::StreamAssistantItem(StreamedAssistantContent::Text(text))) => {
                    yield Ok(text.text);
                }
                Ok(MultiTurnStreamItem::StreamAssistantItem(StreamedAssistantContent::Reasoning(_reasoning))) => {
                    // Ignore reasoning content to prevent it from being displayed or added to history.
                }
                Ok(MultiTurnStreamItem::FinalResponse(_)) => {
                    // Final response received, nothing more to yield
                }
                Err(err) => {
                    yield Err(Error::msg(err.to_string()));
                }
                _ => {}
            }
        }
    }
}

impl Conversation {
    /// Trims history to max_hist and ensures it starts with a User message.
    fn trim_history(&mut self) {
        if let Some(max_hist) = self.max_hist {
            while self.history.len() > max_hist {
                self.history.remove(0);
            }
            // Ensure history starts with a User message to maintain alternation
            while let Some(first) = self.history.first() {
                if matches!(first, Message::User { .. }) {
                    break;
                }
                self.history.remove(0);
            }
        }
    }

    /// Streams a response from the agent, managing history and prefixes.
    pub async fn stream_response(
        &mut self,
        user_message: String,
    ) -> StreamingResult<StreamingCompletionResponse> {
        // Apply prefix and suffix
        let mut full_message = self.presuff.0.clone();
        full_message.push_str(&user_message);
        full_message.push_str(&self.presuff.1);

        // Create user message
        let user_msg = Message::user(full_message.clone());

        // Get stream from agent
        // Pass history *without* the current message to avoid duplication
        let stream = self
            .agent
            .stream_chat(full_message, self.history.clone())
            .await;

        // Store user message as pending until the stream is successfully processed
        self.pending_user_msg = Some(user_msg);

        stream
    }

    /// Appends an assistant response to the conversation history.
    pub fn add_assistant_response(&mut self, response: String) {
        // Add the pending user message first, if it exists
        if let Some(user_msg) = self.pending_user_msg.take() {
            self.history.push(user_msg);
        }
        self.history.push(Message::assistant(response));
        self.trim_history();
    }
}

#[derive(Default, Display, Debug, Clone, EnumIter, EnumString, Subcommand)]
pub enum Talk {
    /// Generic LLM prompt
    #[default]
    #[strum(serialize = "Generic LLM")]
    Generic,
    /// Practice conversation in chosen language
    #[strum(serialize = "Language Practice")]
    LanguagePractice {
        #[arg(value_enum)]
        lang: Lang,
        #[arg(value_enum)]
        level: LangLevel,
    },
    /// Translate subtitles into chosen language
    #[strum(serialize = "Translate Subtitles")]
    TranslateSubs { lang: String },
}

impl Talk {
    /// Initializes a conversation based on the talk type and configuration.
    pub async fn get_conv(&self) -> Result<Conversation, Error> {
        // Load config
        config::ensure_config()?;
        let config_str = std::fs::read_to_string(config::talks_config_path())?;
        let config: TalksConfig = toml::from_str(&config_str)?;

        let talk_name = self.to_string();
        let talk_config = config
            .talks
            .get(&talk_name)
            .ok_or_else(|| Error::msg(format!("Talk '{}' not found in config", talk_name)))?;

        let client = openrouter::Client::from_env()?;
        let mut builder = client.agent(&talk_config.model);
        if let Some(temp) = talk_config.temperature {
            builder = builder.temperature(temp);
        }

        if let Some(params) = &talk_config.additional_params {
            builder = builder.additional_params(params.clone());
        }

        // Interpolate variables in system_prompt and first_msg
        let mut system_prompt = talk_config.system_prompt.clone();
        let mut first_msg = talk_config.first_msg.clone();
        if let Talk::LanguagePractice { lang, level } = self {
            system_prompt = system_prompt.replace("{lang}", &lang.to_string());
            system_prompt = system_prompt.replace("{level}", &level.to_string());
            if let Some(ref mut fm) = first_msg {
                *fm = fm.replace("{lang}", &lang.to_string());
            }
        } else if let Talk::TranslateSubs { lang } = self {
            system_prompt = system_prompt.replace("{lang}", lang);
        }

        // Set the system prompt as a preamble so it is always included
        if !system_prompt.is_empty() {
            builder = builder.preamble(&system_prompt);
        }

        let agent = builder.build();

        let mut history = Vec::new();

        // Handle initial prompt and response generation
        if talk_config.generate_response {
            // Send a minimal trigger message to start the conversation.
            // The instructions are already in the preamble, so we don't need to repeat them.
            let trigger_msg = "";
            let stream = agent
                .stream_chat(trigger_msg.to_string(), history.clone())
                .await;
            let mut response_text = String::new();
            let mut msg_stream = std::pin::pin!(stream_messages(stream));
            while let Some(chunk) = msg_stream.next().await {
                if let Ok(txt) = chunk {
                    response_text.push_str(&txt);
                }
            }
            history.push(Message::user(trigger_msg));
            history.push(Message::assistant(response_text.clone()));
            first_msg = Some(response_text);
        }

        let presuff = (talk_config.prefix.clone(), talk_config.suffix.clone());

        Ok(Conversation {
            agent,
            first_msg,
            presuff,
            max_hist: talk_config.max_hist,
            history,
            pending_user_msg: None,
        })
    }
    /// Checks if the talk type is supported by the Telegram bot.
    pub fn runs_on_bot(&self) -> bool {
        match self {
            Talk::Generic => true,
            Talk::LanguagePractice { .. } => true,
            Talk::TranslateSubs { .. } => false,
        }
    }

    /// Checks if the talk type is supported by the CLI.
    pub fn runs_on_cli(&self) -> bool {
        match self {
            Talk::Generic => true,
            Talk::LanguagePractice { .. } => true,
            Talk::TranslateSubs { .. } => false,
        }
    }
}
