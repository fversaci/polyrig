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
use crate::MyState;
use anyhow::{Error, Result};
use chrono::Duration;
use chrono::prelude::*;
use polyrig::talks::Talk; // get_response, stream_messages
use polyrig::talks::lang_practice::{Lang, LangLevel};
use polyrig::talks::stream_messages;
use rig_core::agent::StreamingResult;
use rig_core::audio_generation::{AudioGenerationError, AudioGenerationModel};
use rig_core::client::ProviderClient;
use rig_core::prelude::{AudioGenerationClient, TranscriptionClient};
use rig_core::providers::openrouter;
use rig_core::providers::openrouter::TranscriptionModel as OpenRouterTranscriptionModel;
use rig_core::transcription::TranscriptionModel;
use std::collections::HashSet;
use std::str::FromStr;
use strum::IntoEnumIterator;
use teloxide::{
    dispatching::{UpdateHandler, dialogue, dialogue::InMemStorage},
    net::Download,
    payloads,
    prelude::*,
    requests::JsonRequest,
    types::{
        ChatAction, InlineKeyboardButton, InlineKeyboardMarkup, InputFile, MessageId, ParseMode,
    },
    utils::command::BotCommands,
};
use tokio_stream::StreamExt;

type MyDialogue = Dialogue<State, InMemStorage<State>>;
type HandlerResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;

const MAX_TG_CHARS: usize = 4000;

#[derive(Default, Clone)]
pub enum State {
    #[default]
    Bouncer,
    Start {
        // my_state: MyState,
    },
    InitTalk {
        my_state: MyState,
        prev: Option<MessageId>,
    },
    ChooseLevel {
        my_state: MyState,
        prev: Option<MessageId>,
        talk: Talk,
    },
    SetLevel {
        my_state: MyState,
        prev: Option<MessageId>,
        talk: Talk,
    },
    ChooseVoiceReply {
        my_state: MyState,
        prev: Option<MessageId>,
        talk: Talk,
    },
    DoTalk {
        my_state: MyState,
        talk: Talk,
    },
}

#[derive(BotCommands, Clone)]
#[command(
    rename_rule = "lowercase",
    description = "These commands are supported:"
)]
enum Command {
    #[command(description = "Show available commands.")]
    Help,
    #[command(description = "(Re)start the menu.")]
    Start,
}

/// Builds the update handler schema for the Telegram bot.
pub fn schema(
    my_state: MyState,
) -> UpdateHandler<Box<dyn std::error::Error + Send + Sync + 'static>> {
    use dptree::case;

    let run_bouncer = move |bot: Bot, dialogue: MyDialogue, msg: Message| {
        bouncer(bot, dialogue, msg, my_state.clone())
    };

    let command_handler = teloxide::filter_command::<Command, _>()
        .branch(case![Command::Help].endpoint(help))
        .branch(case![Command::Start].endpoint(run_bouncer));

    let message_handler =
        Update::filter_message()
            .branch(command_handler)
            .branch(dptree::filter(|msg: Message| msg.voice().is_some()).branch(
                case![State::DoTalk { my_state, talk }].endpoint(do_talk_voice_with_dialogue),
            ))
            .branch(case![State::DoTalk { my_state, talk }].endpoint(do_talk_with_dialogue))
            .branch(dptree::endpoint(invalid_state));

    let callback_query_handler = Update::filter_callback_query()
        .branch(case![State::InitTalk { my_state, prev }].endpoint(init_talk))
        .branch(
            case![State::ChooseLevel {
                my_state,
                prev,
                talk
            }]
            .endpoint(choose_level),
        )
        .branch(
            case![State::SetLevel {
                my_state,
                prev,
                talk
            }]
            .endpoint(set_level),
        )
        .branch(
            case![State::ChooseVoiceReply {
                my_state,
                prev,
                talk
            }]
            .endpoint(choose_voice_reply),
        );

    dialogue::enter::<Update, InMemStorage<State>, State, _>()
        .branch(message_handler)
        .branch(callback_query_handler)
}

/// Displays the list of available bot commands.
async fn help(bot: Bot, msg: Message) -> HandlerResult {
    bot.send_message(msg.chat.id, Command::descriptions().to_string())
        .await?;
    Ok(())
}

/// Handles messages when the dialogue is in an invalid or unhandled state.
async fn invalid_state(bot: Bot, msg: Message) -> HandlerResult {
    bot.send_message(
        msg.chat.id,
        "Unable to handle the message. Type /help to see the usage.",
    )
    .await?;
    Ok(())
}

/// Checks if a chat ID is allowed based on the whitelist.
fn allowed(chat_id: &ChatId, whitelist: &HashSet<ChatId>) -> bool {
    whitelist.contains(chat_id)
}

/// Handles the initial interaction, checks whitelist, and starts the dialogue.
async fn bouncer(bot: Bot, dialogue: MyDialogue, msg: Message, my_state: MyState) -> HandlerResult {
    bot.set_my_commands(Command::bot_commands()).await?;
    // whitelist check
    let chat_id = msg.chat.id;
    let wl = &my_state.my_conf.id_whitelist;
    if !allowed(&chat_id, wl) {
        bot.send_message(chat_id, "Sorry dude, you're not in the whitelist.")
            .await?;
        log::info!("Unknown user: {}", &chat_id);
        return Ok(());
    }
    // set initial state
    dialogue
        .update(State::Start {
            // my_state: my_state.clone(),
        })
        .await?;
    select_talk(bot, dialogue, my_state).await
}

/// Sends a message with an inline keyboard and returns the message ID.
async fn keyb_query(
    bot: &Bot,
    dialogue: &MyDialogue,
    txt_msg: String,
    keyb: InlineKeyboardMarkup,
) -> Result<MessageId> {
    let chat_id = dialogue.chat_id();
    let sent = bot
        .send_message(chat_id, txt_msg)
        .reply_markup(keyb)
        .await?;
    Ok(sent.id)
}

/// Displays the talk selection menu with inline buttons.
async fn select_talk(bot: Bot, dialogue: MyDialogue, my_state: MyState) -> HandlerResult {
    let talks_per_row = 2;
    let talks: Vec<Talk> = Talk::iter().filter(|talk| talk.runs_on_bot()).collect();
    let talks = talks.chunks(talks_per_row).map(|row| {
        row.iter()
            .map(|talk| talk.to_string())
            .map(|talk_cmd| InlineKeyboardButton::callback(talk_cmd.clone(), talk_cmd))
    });
    let txt_msg = "Choose the conversation:".to_string();
    let keyb = InlineKeyboardMarkup::new(talks);
    let prev = Some(keyb_query(&bot, &dialogue, txt_msg, keyb).await?);
    dialogue.update(State::InitTalk { my_state, prev }).await?;
    Ok(())
}

/// Deletes a previous message containing inline buttons.
async fn clean_buttons(bot: Bot, chat_id: ChatId, m_id: Option<MessageId>) -> Result<()> {
    // clean old buttons?
    if let Some(m_id) = m_id {
        bot.delete_message(chat_id, m_id).await?;
    }
    Ok(())
}

/// Handles the callback query for initializing a selected talk.
async fn init_talk(
    bot: Bot,
    dialogue: MyDialogue,
    q: CallbackQuery,
    tup_state: (MyState, Option<MessageId>),
) -> HandlerResult {
    let (my_state, prev) = tup_state;
    let chat_id = dialogue.chat_id();
    clean_buttons(bot.clone(), chat_id, prev).await?;
    let talk = q.data.unwrap_or_default();
    let talk = Talk::from_str(&talk).unwrap_or_default();
    match talk {
        Talk::LanguagePractice { .. } => choose_lang(bot, dialogue, talk, my_state).await,
        // Talk::Generic
        _ => start_talk(bot, dialogue, talk, my_state).await,
    }
}

/// Displays the language selection menu for language practice talks.
async fn choose_lang(
    bot: Bot,
    dialogue: MyDialogue,
    talk: Talk,
    my_state: MyState,
) -> HandlerResult {
    let langs_per_row = 3;
    let langs: Vec<Lang> = Lang::iter().collect();
    let langs = langs.chunks(langs_per_row).map(|row| {
        row.iter()
            .map(|lang| lang.to_string())
            .map(|lang_cmd| InlineKeyboardButton::callback(lang_cmd.clone(), lang_cmd))
    });
    let txt_msg = "Choose the language:".to_string();
    let keyb = InlineKeyboardMarkup::new(langs);
    let prev = Some(keyb_query(&bot, &dialogue, txt_msg, keyb).await?);
    dialogue
        .update(State::ChooseLevel {
            my_state,
            prev,
            talk,
        })
        .await?;
    Ok(())
}

/// Displays the proficiency level selection menu.
async fn choose_level(
    bot: Bot,
    dialogue: MyDialogue,
    q: CallbackQuery,
    tup_state: (MyState, Option<MessageId>, Talk),
) -> HandlerResult {
    let (my_state, prev, mut talk) = tup_state;
    let chat_id = dialogue.chat_id();
    clean_buttons(bot.clone(), chat_id, prev).await?;
    let new_lang = q.data.unwrap_or_default();
    let new_lang = Lang::from_str(&new_lang).unwrap_or_default();
    if let Talk::LanguagePractice { ref mut lang, .. } = talk {
        *lang = new_lang;
    }
    let levs_per_row = 2;
    let levs: Vec<LangLevel> = LangLevel::iter().collect();
    let levs = levs.chunks(levs_per_row).map(|row| {
        row.iter()
            .map(|lev| lev.to_string())
            .map(|lev_cmd| InlineKeyboardButton::callback(lev_cmd.clone(), lev_cmd))
    });
    let txt_msg = "Choose your level:".to_string();
    let keyb = InlineKeyboardMarkup::new(levs);
    let prev = Some(keyb_query(&bot, &dialogue, txt_msg, keyb).await?);
    dialogue
        .update(State::SetLevel {
            my_state,
            prev,
            talk,
        })
        .await?;
    Ok(())
}

/// Sets the proficiency level and asks about voice reply preference.
async fn set_level(
    bot: Bot,
    dialogue: MyDialogue,
    q: CallbackQuery,
    tup_state: (MyState, Option<MessageId>, Talk),
) -> HandlerResult {
    let (my_state, prev, mut talk) = tup_state;
    let chat_id = dialogue.chat_id();
    clean_buttons(bot.clone(), chat_id, prev).await?;
    let new_lev = q.data.unwrap_or_default();
    let new_lev = LangLevel::from_str(&new_lev).unwrap_or_default();
    if let Talk::LanguagePractice { ref mut level, .. } = talk {
        *level = new_lev;
    }
    choose_voice_reply_menu(bot, dialogue, my_state, talk).await
}

/// Displays the voice reply choice menu.
async fn choose_voice_reply_menu(
    bot: Bot,
    dialogue: MyDialogue,
    my_state: MyState,
    talk: Talk,
) -> HandlerResult {
    let keyb = InlineKeyboardMarkup::new([[
        InlineKeyboardButton::callback("Yes", "voice_yes"),
        InlineKeyboardButton::callback("No", "voice_no"),
    ]]);
    let txt_msg = "Enable voice replies?".to_string();
    let prev = Some(keyb_query(&bot, &dialogue, txt_msg, keyb).await?);
    dialogue
        .update(State::ChooseVoiceReply {
            my_state,
            prev,
            talk,
        })
        .await?;
    Ok(())
}

/// Handles the voice reply choice callback.
async fn choose_voice_reply(
    bot: Bot,
    dialogue: MyDialogue,
    q: CallbackQuery,
    tup_state: (MyState, Option<MessageId>, Talk),
) -> HandlerResult {
    let (mut my_state, prev, talk) = tup_state;
    let chat_id = dialogue.chat_id();
    clean_buttons(bot.clone(), chat_id, prev).await?;
    let voice_reply = q.data.unwrap_or_default() == "voice_yes";
    my_state.voice_reply = voice_reply;
    start_talk(bot, dialogue, talk, my_state).await
}

/// Initializes the conversation agent and sends the first message.
async fn start_talk(
    bot: Bot,
    dialogue: MyDialogue,
    talk: Talk,
    my_state: MyState,
) -> HandlerResult {
    let chat_id = dialogue.chat_id();
    log::info!(
        "User: {} Talk: {:?} VoiceReply: {}",
        &chat_id,
        &talk,
        my_state.voice_reply
    );
    let conversation = talk.get_conv().await?;

    // Send the first message if it exists, splitting if necessary
    if let Some(first_msg) = &conversation.first_msg {
        send_text_chunks(bot.clone(), chat_id, first_msg).await?;
    }

    // Update dialogue state with the conversation
    dialogue
        .update(State::DoTalk {
            my_state: MyState {
                my_conf: my_state.my_conf,
                agent: Some(conversation.agent),
                history: conversation.history,
                presuff: conversation.presuff,
                max_hist: conversation.max_hist,
                voice_reply: my_state.voice_reply,
                transcription_model: my_state.transcription_model.clone(),
                tts_model: my_state.tts_model.clone(),
                tts_voice: my_state.tts_voice.clone(),
                tts_format: my_state.tts_format.clone(),
            },
            talk,
        })
        .await?;
    Ok(())
}

/// Handles user messages during an active conversation.
async fn do_talk_with_dialogue(
    bot: Bot,
    msg: Message,
    dialogue: MyDialogue,
    tup_state: (MyState, Talk),
) -> HandlerResult {
    let (my_state, talk) = tup_state;
    let user_message = msg
        .text()
        .ok_or(Error::msg("## Error in message! ##"))?
        .to_string();
    do_talk(bot, dialogue, my_state, user_message, talk).await
}

/// Handles voice messages during an active conversation.
/// Downloads the voice file, transcribes it via OpenAI Whisper,
/// then feeds the transcribed text into the conversation.
async fn do_talk_voice_with_dialogue(
    bot: Bot,
    msg: Message,
    dialogue: MyDialogue,
    tup_state: (MyState, Talk),
) -> HandlerResult {
    let (my_state, talk) = tup_state;
    let chat_id = msg.chat.id;
    let voice = msg.voice().ok_or(Error::msg("No voice in message"))?;

    bot.send_chat_action(chat_id, ChatAction::Typing).await?;

    let tg_file = bot.get_file(voice.file.id.clone()).await?;
    let mut audio_data = Vec::new();
    bot.download_file(&tg_file.path, &mut audio_data).await?;

    let openrouter_client = openrouter::Client::from_env()?;
    let whisper: OpenRouterTranscriptionModel =
        openrouter_client.transcription_model(&my_state.transcription_model);

    let language = match &talk {
        Talk::LanguagePractice { lang, .. } => Some(lang.iso_639_1().to_string()),
        _ => None,
    };

    let mut request = whisper
        .transcription_request()
        .data(audio_data)
        .filename(Some("voice.ogg".to_string()));
    if let Some(lang_code) = language {
        request = request.language(lang_code);
    }

    let transcription_result =
        tokio::spawn(async move { request.send().await.map(|r| r.text) }).await;

    let user_message = match transcription_result {
        Ok(Ok(text)) => text,
        Ok(Err(e)) => {
            log::error!("Transcription error: {}", e);
            bot.send_message(
                chat_id,
                "Failed to transcribe the voice message. Please send a text message instead.",
            )
            .await?;
            return Ok(());
        }
        Err(join_err) => {
            log::error!("Transcription panicked: {}", join_err);
            bot.send_message(
                chat_id,
                "Failed to transcribe the voice message. Please send a text message instead.",
            )
            .await?;
            return Ok(());
        }
    };

    if user_message.trim().is_empty() {
        bot.send_message(
            chat_id,
            "Could not transcribe the voice message. Please send a text message instead.",
        )
        .await?;
        return Ok(());
    }

    do_talk(bot, dialogue, my_state, user_message, talk).await
}

/// Builds a WAV header for 16-bit mono PCM audio at the given sample rate.
fn wav_header(data_len: usize, sample_rate: u32) -> Vec<u8> {
    let bits_per_sample: u16 = 16;
    let num_channels: u16 = 1;
    let byte_rate = sample_rate * u32::from(num_channels) * u32::from(bits_per_sample) / 8;
    let block_align = num_channels * bits_per_sample / 8;
    let data_size = data_len as u32;
    let chunk_size = 36 + data_size;

    let mut hdr = Vec::with_capacity(44);
    hdr.extend_from_slice(b"RIFF");
    hdr.extend_from_slice(&chunk_size.to_le_bytes());
    hdr.extend_from_slice(b"WAVE");
    hdr.extend_from_slice(b"fmt ");
    hdr.extend_from_slice(&16u32.to_le_bytes()); // subchunk1 size
    hdr.extend_from_slice(&1u16.to_le_bytes());  // PCM
    hdr.extend_from_slice(&num_channels.to_le_bytes());
    hdr.extend_from_slice(&sample_rate.to_le_bytes());
    hdr.extend_from_slice(&byte_rate.to_le_bytes());
    hdr.extend_from_slice(&block_align.to_le_bytes());
    hdr.extend_from_slice(&bits_per_sample.to_le_bytes());
    hdr.extend_from_slice(b"data");
    hdr.extend_from_slice(&data_size.to_le_bytes());
    hdr
}

/// Generates audio from text using OpenRouter TTS and sends it as a voice message.
/// `tts_format` is "mp3" or "pcm" — determines the response format and output file type.
async fn send_voice_reply(
    bot: Bot,
    chat_id: ChatId,
    text: &str,
    tts_model: &str,
    tts_voice: &str,
    tts_format: &str,
) {
    if text.trim().is_empty() {
        return;
    }
    let text = text.to_string();
    let tts_model = tts_model.to_string();
    let tts_voice = tts_voice.to_string();
    let tts_format = tts_format.to_string();
    let is_pcm = tts_format == "pcm";
    let tts_result = tokio::spawn(async move {
        let openrouter_client = openrouter::Client::from_env()
            .map_err(|e| AudioGenerationError::RequestError(Box::new(e)))?;
        let tts = openrouter_client.audio_generation_model(&tts_model);
        let mut req = tts.audio_generation_request().text(&text).voice(&tts_voice);
        if is_pcm {
            req = req.additional_params(serde_json::json!({"response_format": "pcm"}));
        }
        req.send().await.map(|r| r.audio)
    })
    .await;

    match tts_result {
        Ok(Ok(audio_data)) => {
            let voice_file = if is_pcm {
                // Gemini TTS returns raw PCM (16-bit, 24000 Hz, mono); wrap in WAV header.
                let mut wav = wav_header(audio_data.len(), 24000);
                wav.extend_from_slice(&audio_data);
                InputFile::memory(wav).file_name("reply.wav")
            } else {
                InputFile::memory(audio_data).file_name("reply.mp3")
            };
            if let Err(e) = bot.send_voice(chat_id, voice_file).await {
                log::error!("Failed to send voice reply: {}", e);
            }
        }
        Ok(Err(e)) => {
            log::error!("TTS generation error: {}", e);
        }
        Err(join_err) => {
            log::error!("TTS generation panicked: {}", join_err);
        }
    }
}

/// Core logic for sending a user message to the LLM and streaming the response.
async fn do_talk(
    bot: Bot,
    dialogue: MyDialogue,
    my_state: MyState,
    user_message: String,
    talk: Talk,
) -> HandlerResult {
    let chat_id = dialogue.chat_id();

    let agent = my_state
        .agent
        .as_ref()
        .ok_or_else(|| Error::msg("No agent available"))?;

    let mut temp_conv = polyrig::talks::Conversation {
        agent: agent.clone(),
        first_msg: None,
        presuff: my_state.presuff.clone(),
        max_hist: my_state.max_hist,
        history: my_state.history.clone(),
        pending_user_msg: None,
    };

    let stream = temp_conv.stream_response(user_message).await;
    let response = send_stream(bot.clone(), chat_id, stream).await?;

    temp_conv.add_assistant_response(response.clone());

    if my_state.voice_reply {
        send_voice_reply(
            bot.clone(),
            chat_id,
            &response,
            &my_state.tts_model,
            &my_state.tts_voice,
            &my_state.tts_format,
        )
        .await;
    }

    dialogue
        .update(State::DoTalk {
            my_state: MyState {
                my_conf: my_state.my_conf,
                agent: Some(agent.clone()),
                history: temp_conv.history,
                presuff: my_state.presuff,
                max_hist: my_state.max_hist,
                voice_reply: my_state.voice_reply,
                transcription_model: my_state.transcription_model.clone(),
                tts_model: my_state.tts_model.clone(),
                tts_voice: my_state.tts_voice.clone(),
                tts_format: my_state.tts_format.clone(),
            },
            talk,
        })
        .await?;

    Ok(())
}

/// Sends a text message, splitting it into chunks if it exceeds Telegram's limit.
async fn send_text_chunks(bot: Bot, chat_id: ChatId, msg: &str) -> Result<()> {
    if msg.is_empty() {
        log::warn!(
            "send_text_chunks called with empty message, skipping (chat_id={})",
            chat_id
        );
        return Ok(());
    }
    let chars: Vec<char> = msg.chars().collect();
    if chars.len() <= MAX_TG_CHARS {
        // For short messages, try markdown first, then plain text
        let md = payloads::SendMessage::new(chat_id, msg);
        type Sender = JsonRequest<payloads::SendMessage>;
        let sent = Sender::new(bot.clone(), md.clone().parse_mode(ParseMode::Markdown)).await;
        if let Err(e) = sent {
            Sender::new(bot, md).await?;
            log::debug!("Cannot parse markdown: {}", e);
        }
        return Ok(());
    }

    // Split into chunks, trying to break at sentence boundaries
    let mut start = 0;
    let mut part_num = 1;
    let total_parts = chars.len().div_ceil(MAX_TG_CHARS);
    while start < chars.len() {
        // Calculate end index, ensuring it doesn't exceed the slice length
        let mut end_char_idx = std::cmp::min(start + MAX_TG_CHARS, chars.len());
        if end_char_idx < chars.len() {
            // Try to cut at a sentence boundary within the last 200 chars
            let search_len = std::cmp::min(200, end_char_idx - start);
            let search_start = end_char_idx - search_len;
            let slice: String = chars[search_start..end_char_idx].iter().collect();
            // Look for common sentence terminators
            if let Some(pos) = slice.rfind(['.', '!', '?', '。', '？', '！', '؛', '۔']) {
                // Convert byte position to char offset within the slice
                let char_offset = slice[..pos].chars().count();
                end_char_idx = search_start + char_offset + 1;
                // Ensure we don't go backwards
                end_char_idx = std::cmp::max(end_char_idx, start + 1);
            }
        }
        let chunk: String = chars[start..end_char_idx].iter().collect();
        // Use plain text for all chunks to avoid markdown issues in split messages
        let suffix = if part_num < total_parts {
            "\n(...)"
        } else {
            ""
        };
        bot.send_message(chat_id, format!("{}{}", chunk, suffix))
            .await?;
        start = end_char_idx;
        part_num += 1;
    }
    Ok(())
}

/// Updates an existing message, attempting Markdown parsing first.
async fn update_markdown(bot: Bot, chat_id: ChatId, m_id: MessageId, msg: &str) -> Result<()> {
    if msg.is_empty() {
        log::warn!(
            "update_markdown called with empty message, skipping (chat_id={}, m_id={:?})",
            chat_id,
            m_id
        );
        return Ok(());
    }
    let md = payloads::EditMessageText::new(chat_id, m_id, msg);
    type Sender = JsonRequest<payloads::EditMessageText>;
    let sent = Sender::new(bot.clone(), md.clone().parse_mode(ParseMode::Markdown)).await;
    // If markdown cannot be parsed, send it as raw text
    if let Err(e) = sent {
        Sender::new(bot, md).await?;
        log::debug!("Cannot parse markdown: {}", e);
    }

    Ok(())
}

/// State for managing multiple message updates during streaming
struct StreamState {
    messages: Vec<(MessageId, String)>, // (message_id, content_without_suffix)
}

impl StreamState {
    fn new() -> Self {
        StreamState {
            messages: Vec::new(),
        }
    }

    /// Gets the current message ID to update and its base content
    fn current_target(&self) -> Option<(MessageId, &str)> {
        self.messages
            .last()
            .map(|(id, content)| (*id, content.as_str()))
    }

    /// Adds a new message to track
    fn push_message(&mut self, m_id: MessageId) {
        self.messages.push((m_id, String::new()));
    }

    /// Updates the content of the current (last) message
    fn update_current_content(&mut self, new_content: String) {
        if let Some(last) = self.messages.last_mut() {
            last.1 = new_content;
        }
    }

    /// Constructs the full text for a message at given index, including suffix if needed
    fn get_message_text(&self, idx: usize, is_final: bool) -> String {
        if idx >= self.messages.len() {
            return String::new();
        }
        let content = &self.messages[idx].1;
        // Add suffix if this is not the last message OR if it's the last message but not final
        let has_more = idx < self.messages.len() - 1 || !is_final;
        if has_more && !content.is_empty() {
            format!("{}\n(...)", content)
        } else {
            content.clone()
        }
    }

    /// Checks if adding more text would exceed the limit for the current message
    fn would_exceed_limit(&self, additional_chars: usize) -> bool {
        if let Some(last) = self.messages.last() {
            let current_len = last.1.chars().count();
            // When checking during streaming, we add the suffix to current message
            let suffix_len = 7; // "\n(...)" is 7 chars
            current_len + additional_chars + suffix_len > MAX_TG_CHARS
        } else {
            false
        }
    }
}

/// Streams the LLM response to a chat message with live updates, splitting across multiple messages if needed.
async fn send_stream<M>(bot: Bot, chat_id: ChatId, stream: StreamingResult<M>) -> Result<String> {
    // Initialize stream state
    let mut stream_state = StreamState::new();

    // send initial message (message zero)
    let zero = bot.send_message(chat_id, "(...)").await?;
    stream_state.push_message(zero.id);

    // send updates
    let mut messages = Box::pin(stream_messages(stream));
    let mut full_response = String::new();
    let mut oldtime = Utc::now();
    let mintime = Duration::milliseconds(2500);

    while let Some(chunk) = messages.next().await {
        match chunk {
            Ok(delta) => {
                full_response.push_str(&delta);

                // Check if we need to create a new message because current one is getting too large
                if stream_state.would_exceed_limit(delta.chars().count()) {
                    // Finalize current message with suffix (it will have more messages after)
                    if let Some((current_id, _)) = stream_state.current_target() {
                        let current_content = stream_state.messages.last().unwrap().1.clone();
                        let final_text = format!("{}\n(...)", current_content);
                        update_markdown(bot.clone(), chat_id, current_id, &final_text).await?;
                    }

                    // Create new message for remaining content
                    let new_msg = bot.send_message(chat_id, "(...)").await?;
                    stream_state.push_message(new_msg.id);

                    // Update the new message with the delta that didn't fit in previous
                    stream_state.update_current_content(delta.clone());
                } else {
                    // Update current message content
                    if let Some(last) = stream_state.messages.last_mut() {
                        last.1.push_str(&delta);
                    }
                }

                // Send updates periodically
                let now = Utc::now();
                if now - oldtime > mintime {
                    // Update all messages (in practice, mostly the last one changes)
                    // Pass is_final=false because streaming is still ongoing
                    for (idx, (m_id, _)) in stream_state.messages.iter().enumerate() {
                        let text = stream_state.get_message_text(idx, false);
                        // Only update if within limit (should always be true by construction)
                        if text.chars().count() <= MAX_TG_CHARS {
                            update_markdown(bot.clone(), chat_id, *m_id, &text).await?;
                        }
                    }
                    oldtime = now;
                }
            }
            Err(e) => {
                // Append error to the last message
                if let Some((last_id, _)) = stream_state.current_target() {
                    let error_text = format!("\n\n**Error:** {}", e);
                    if let Some(last) = stream_state.messages.last_mut() {
                        last.1.push_str(&error_text);
                        // Update with is_final=true to remove suffix from last message
                        let text =
                            stream_state.get_message_text(stream_state.messages.len() - 1, true);
                        update_markdown(bot.clone(), chat_id, last_id, &text).await?;
                    }
                }
                break;
            }
        }
    }

    // Finalize: if response is empty, show placeholder
    if full_response.is_empty() {
        log::warn!("Stream response was empty for chat_id={}", chat_id);
        full_response.push_str("-- ␃ --");
        if let Some((last_id, _)) = stream_state.current_target() {
            stream_state.update_current_content(full_response.clone());
            // Update with is_final=true to remove suffix from last message
            let text = stream_state.get_message_text(stream_state.messages.len() - 1, true);
            update_markdown(bot.clone(), chat_id, last_id, &text).await?;
        }
    } else {
        // Remove the "..." suffix from the last message (it's the final one, no continuation)
        if let Some((last_id, _)) = stream_state.current_target() {
            // Update with is_final=true to remove suffix from last message
            let text = stream_state.get_message_text(stream_state.messages.len() - 1, true);
            update_markdown(bot.clone(), chat_id, last_id, &text).await?;
        }
    }

    Ok(full_response)
}
