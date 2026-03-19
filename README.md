# PolyRig: A Rust Companion for Language Practice, Powered by LLMs

## Overview

PolyRig is a Rust toolkit for language practice and processing. It provides interactive tools — a CLI chat, a Telegram bot with voice conversations, a subtitle translator, and an audio transcriber — all powered by LLMs via `rig-core` and OpenRouter.

1.  **CLI (`polyrig`)**: An interactive command-line interface for generic chat and language practice.
2.  **Telegram Bot (`polyrig-bot`)**: A Telegram bot for interacting with the LLM. Supports **voice conversations** for language practice — send audio messages to practice a foreign language, and optionally receive voice replies from the model.
3.  **Subtitle Translator (`translate-subs`)**: A CLI tool to translate `.srt` subtitle files using LLMs.
4.  **Speech to Text (`speech-to-text`)**: A CLI tool to transcribe or translate audio files using OpenAI's Whisper model.

## Configuration

### Setting the API Keys

This project uses OpenRouter to access various LLMs. You need to set the `OPENROUTER_API_KEY` environment variable:

```bash
export OPENROUTER_API_KEY="your_openrouter_api_key_here"
```

**Note:** The `speech-to-text` binary uses the OpenAI API directly. To use it, you must set the `OPENAI_API_KEY` environment variable:

```bash
export OPENAI_API_KEY="your_openai_api_key_here"
```

### Talk Configuration (`conf/talks.toml`)

The behavior of the different "Talks" (conversation modes) is defined in `conf/talks.toml`. This file specifies:
- **System Prompt**: The instructions given to the model.
- **Model**: The specific LLM to use (e.g., `deepseek/deepseek-v4-flash:nitro`, `google/gemma-4-26b-a4b-it:nitro`).
- **Parameters**: Settings like temperature, history length, and message delimiters.

### Bot Configuration (`conf/defaults.toml`)

The Telegram bot requires a configuration file at `conf/defaults.toml` for settings like user whitelisting and audio model selection. Copy `conf/defaults.toml.template` to `conf/defaults.toml` to get started.

```toml
id_whitelist = [
  123456789,  # user_id_1
  987654321,  # user_id_2
]

# Optional: override the transcription model (used for voice messages)
# transcription_model = "openai/gpt-4o-mini-transcribe"

# Optional: override the TTS model (used for voice replies)
# tts_model = "openai/gpt-4o-mini-tts-2025-12-15"

# Optional: override the TTS voice (varies by model)
# tts_voice = "marin"
```

*Note: At least one user ID must be listed; an empty whitelist blocks all access. All audio model fields have sensible defaults.*

## Installation

Assuming you have a Rust toolchain installed, build the project with:

```bash
cargo build --release
```

## Usage

The CLI allows you to interact with the LLM from your terminal.

**Generic Chat:**
```bash
polyrig generic
```

**Language Practice:**
```bash
polyrig language-practice german advanced
```

*Note: Press Enter twice to send your message. Send an empty message to leave the chat.*

After each response, an **interactive scrollable markdown view** of the full conversation is displayed. Use `↑`/`↓`, `PgUp`/`PgDn`, `j`/`k`, or the mouse wheel to scroll. Press `q` or `Esc` to exit. The latest response is re-rendered with markdown formatting after exiting the scroll view.

### 2. Telegram Bot (`polyrig-bot`)

To run the Telegram bot, you need a bot token from BotFather.

```bash
TELOXIDE_TOKEN="your_telegram_bot_token" polyrig-bot
```

#### Voice Conversations for Language Practice

The bot supports **full voice conversations** — ideal for practicing a foreign language:

- **Send voice messages** (audio) to the bot — they are automatically transcribed using OpenAI Whisper (via rig-core's OpenRouter provider) and sent to the LLM.
- **Receive voice replies** — when enabled, the bot generates spoken responses using text-to-speech, so you can listen instead of reading.
- Choose any language and proficiency level (e.g., German Advanced) for targeted practice with grammar correction.

To enable voice replies, select the language practice talk in the bot and confirm when prompted "Enable voice replies?".

### 3. Subtitle Translator (`translate-subs`)

Translate `.srt` files from one language to another.

```bash
translate-subs input.srt output.srt english
```

**Debug Mode:**
To enable debug logging (shows the first 20 lines of requests and responses from OpenRouter), set the `RUST_LOG` environment variable to `debug`:

```bash
RUST_LOG=debug translate-subs input.srt output.srt english
```

For more options (like chunk size), run:
```bash
translate-subs --help
```

### 4. Speech to Text (`speech-to-text`)

Transcribe or translate audio files using OpenAI's Whisper models.

**Transcription:**
```bash
speech-to-text input.mp3 output.txt
```

**Translation (to English):**
```bash
speech-to-text input.mp3 output.txt --to-eng
```

**SRT Output:**
```bash
speech-to-text input.mp3 output.srt --srt
```

For more options, run:
```bash
speech-to-text --help
```

## Architecture

For a detailed overview of the project structure, see [docs/architecture.md](docs/architecture.md).

## Author

Francesco Versaci <francesco.versaci@gmail.com>

## License

Licensed under the GNU Affero General Public License v3.0.
