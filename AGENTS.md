# AGENTS.md - Guidelines for Working in This Repository

## Project Overview

PolyRig is a Rust project providing LLM-powered conversational interfaces via `rig-core` (using OpenRouter). It consists of a core library and four binaries:

1. **CLI (`polyrig`)**: Interactive command-line interface for generic chat and language practice.
2. **Telegram Bot (`polyrig-bot`)**: Telegram bot for LLM interaction.
3. **Subtitle Translator (`translate-subs`)**: CLI tool to translate `.srt` subtitle files.
4. **Speech to Text (`speech-to-text`)**: CLI tool to transcribe/translate audio using OpenAI's Whisper.

## Essential Commands

### Build & Run

```bash
# Build all binaries in release mode
cargo build --release

# Run specific binary
cargo run --bin polyrig -- [subcommand]
cargo run --bin polyrig-bot
cargo run --bin translate-subs -- [args]
cargo run --bin speech-to-text -- [args]

# Run bot with token (from run-bot.sh)
TELOXIDE_TOKEN="..." RUST_LOG=info cargo run --bin polyrig-bot
```

### Testing & Linting

```bash
# No tests currently exist in the project

# Check code without building
cargo check

# Format code
cargo fmt

# Lint
cargo clippy
```

### Common CLI Usage

```bash
# Generic chat
polyrig generic

# Language practice (e.g., German B2)
polyrig language-practice german b2

# Note: Press Enter twice to send message in CLI
```

## Environment Variables

- **`OPENROUTER_API_KEY`**: Required for all binaries except `speech-to-text`. Set this to use OpenRouter models.
- **`OPENAI_API_KEY`**: Required only for `speech-to-text` (uses OpenAI Whisper directly).
- **`TELOXIDE_TOKEN`**: Required for the Telegram bot (from BotFather).
- **`RUST_LOG`**: Controls logging level (e.g., `info`, `debug`).

## Configuration Files

### `conf/talks.toml`

Defines system prompts, models, and parameters for each conversation mode ("Talk"). Key fields:

- `system_prompt`: Instructions for the model (supports `{lang}` and `{level}` interpolation for language practice)
- `model`: Model identifier (e.g., `deepseek/deepseek-v4-flash:nitro`, `google/gemma-4-26b-a4b-it:nitro`)
- `prefix`/`suffix`: Delimiters added to user messages (used for language practice to wrap text in `<correct_me>` tags)
- `max_hist`: Maximum conversation history length (0 = no history)
- `first_msg`: Optional initial message shown to the user on startup
- `generate_response`: If `true`, generates an initial response on startup by sending a trigger message to the model
- `temperature`: Model temperature (optional)
- `additional_params`: Extra parameters like `response_format` for JSON schema enforcement

### `conf/defaults.toml`

Bot-specific settings:

- `id_whitelist`: List of allowed Telegram user IDs. An empty list blocks all access; at least one ID must be present. This file is git-ignored; create from `defaults.toml.template`.
- `transcription_model`: Model for voice message transcription (default: `openai/gpt-4o-mini-transcribe`).
- `tts_model`: Model for voice reply generation (default: `google/gemini-3.1-flash-tts-preview`).
- `tts_voice`: Voice name for TTS output (default: `Sulafat`; available voices depend on the TTS model).

## Architecture & Data Flow

### Core Library (`src/lib.rs`, `src/talks.rs`)

- **`Talk` enum**: Defines conversation modes (`Generic`, `LanguagePractice`, `TranslateSubs`). Derives `clap::Subcommand` for CLI argument parsing.
- **`Conversation` struct**: Manages agent, message history, and streaming. Key methods:
  - `stream_response()`: Sends user message, returns streaming result, adds user message to history, trims history.
  - `add_assistant_response()`: Appends assistant response to history after streaming completes.
  - `trim_history()`: Enforces `max_hist` limit and ensures history starts with a `User` message.
- **`stream_messages()`**: Converts `rig-core` streaming response into `Stream<Item = Result<String>>`, filtering out reasoning content.

### CLI Binary (`src/bin/polyrig/`)

- Uses `clap` to parse `Talk` subcommand.
- Uses `rustyline` for interactive input.
- Reads multi-line messages until empty line (user presses Enter twice).
- Streams responses raw (no formatting) for smooth live display.
- After streaming completes, shows an **interactive scrollable markdown view** of the full conversation (via `view_markdown.rs` using `termimad`), supporting PgUp/PgDn, arrow keys, and mouse wheel.
- After exiting scroll view, re-renders the latest response with markdown formatting (using `termimad::MadSkin`).

### Telegram Bot (`src/bin/polyrig-bot/`)

- **State machine** (in `telegram.rs`):
  - `Bouncer`: Checks user whitelist.
  - `Start`: Shows main menu.
  - `InitTalk`: User selects talk type via inline keyboard.
  - `ChooseLevel`/`SetLevel`: For language practice, select proficiency level.
  - `DoTalk`: Active conversation, streams responses with live updates.
- Uses `teloxide`'s dialogue system with in-memory storage.
- Streams responses character-by-character to Telegram with editing.

### Subtitle Translator (`src/bin/translate-subs.rs`)

- Parses SRT files using `subtp` crate.
- Chunks subtitles (default 256 blocks), attempting to split at sentence boundaries.
- Converts each chunk to JSON with random 5-char labels, sends to LLM, parses translated JSON back.
- Retries up to 3 times on errors, creating a fresh translator agent each retry.
- Handles text spreading across multiple subtitle frames when merging occurs.

### Speech to Text (`src/bin/speech-to-text.rs`)

- Uses `async-openai` directly (not OpenRouter).
- Supports transcription (`--to-eng` flag for translation to English).
- Output formats: JSON (default, with diarization) or SRT (`--srt` flag).
- Optional prompt to guide style.

## Code Patterns & Conventions

### Error Handling

- Uses `anyhow::Result` throughout for simple error propagation.
- Errors are typically logged or printed, not extensively handled.

### Async Patterns

- All binaries use `#[tokio::main]`.
- Streaming uses `tokio_stream::StreamExt`.
- `std::pin::pin!` used to pin streams before polling.

### History Management

- Conversation history is a `Vec<Message>` (from `rig-core`).
- History is trimmed **after** each exchange to maintain a bounded context window.
- `max_hist = 0` means no history is kept (each exchange is independent).

### Message Delimiters (Prefix/Suffix)

- The `presuff` tuple `(prefix, suffix)` is applied to user messages before sending to the model.
- Used primarily for language practice to wrap user text in `<correct_me>...</correct_me>` tags.
- The delimiters are configured per talk type in `conf/talks.toml`.

## Important Gotchas

### 1. Edition 2024

The `Cargo.toml` specifies `edition = "2024"`. This is a future Rust edition (not yet stable as of 2025). You may need a nightly toolchain or may encounter compilation issues on stable.

### 2. Deprecated ParseMode::Markdown

`src/bin/polyrig-bot/telegram.rs` uses `ParseMode::Markdown` (deprecated in favor of `MarkdownV2`). This is acceptable — `MarkdownV2` requires escaping special characters which is error-prone for LLM output, so the legacy mode is kept intentionally.

### 3. Config File Paths

Config files are loaded with relative paths (`"conf/talks.toml"`, `"conf/defaults.toml"`). The working directory must be the project root, or these paths will fail.

### 4. Message Delimiters and Empty Input

In the CLI, the `read_msg` function reads until an empty line. If the user just presses Enter (empty message), it returns `None` and the loop exits. This is intentional behavior.

### 5. History Asymmetry

`stream_response()` adds the user message to history **before** streaming the response, but the response is only added via a separate `add_assistant_response()` call after streaming completes. If streaming fails mid-response, the user message is in history but the (partial) assistant response is not, which could cause asymmetry.

### 6. Subtitle Random Labels

The subtitle translator generates random 5-character alphanumeric labels for each subtitle block. These are used as JSON keys to prevent the LLM from reordering content. The labels must be preserved exactly in the translated output.

### 7. Speech-to-Text Model

The transcription uses `gpt-4o-transcribe-diarize` model (not standard Whisper) when outputting JSON with diarization. The translation uses `whisper-1`.

### 8. No Test Suite

The project has no tests. All validation is manual or through runtime usage.

## File Structure

```
src/
├── lib.rs                          # Library root (just exports talks module)
├── talks.rs                        # Core: Talk enum, Conversation struct, streaming
└── talks/
    └── lang_practice.rs            # Lang and LangLevel enums
src/bin/
├── speech-to-text.rs               # Speech-to-text binary (OpenAI direct)
├── translate-subs.rs               # Subtitle translator binary
└── polyrig/
    ├── main.rs                     # CLI binary entry point
    └── view_markdown.rs            # Interactive scrollable markdown viewer
└── polyrig-bot/
    ├── main.rs                     # Bot entry point, config loading
    └── telegram.rs                 # Bot state machine and handlers
conf/
├── talks.toml                      # Conversation mode configurations
└── defaults.toml.template          # Bot config template (defaults.toml is git-ignored)
docs/
├── architecture.md                 # High-level architecture overview
├── issues.md                       # Known issues
└── srt-translation.md              # Details on subtitle translation
```

## Dependencies

Key crates:

- **`rig-core`**: LLM abstraction layer (OpenRouter provider). Supports chat, transcription, and TTS/audio generation.
- **`async-openai`**: Direct OpenAI API client (for speech-to-text binary).
- **`teloxide`**: Telegram bot framework.
- **`clap`**: CLI argument parsing (with derive macros).
- **`rustyline`**: Readline-like input for CLI.
- **`tokio`**: Async runtime (full features).
- **`serde`/`serde_json`**: Serialization.
- **`toml`**: Config file parsing.
- **`strum`/`strum_macros`**: Enum utilities (derive Display, EnumString, EnumIter).
- **`subtp`**: SRT subtitle parsing.
- **`chrono`**: Date/time handling.
- **`anyhow`**: Error handling.
- **`log`/`pretty_env_logger`/`env_logger`**: Logging.
- **`termimad`**: Markdown rendering and interactive scrollable views (CLI).
- **`async-stream`**: Async stream macros.
- **`rand`**: Random label generation for subtitle translation.
