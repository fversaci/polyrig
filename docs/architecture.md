# PolyRig Architecture

> **Project**: [PolyRig](https://github.com/fversaci/polyrig)
> **License**: AGPL-3.0
> **Author**: Francesco Versaci

---

## 1. Overview

PolyRig is a Rust workspace providing multiple LLM-powered interfaces over a shared core library. It wraps [rig-core](https://crates.io/crates/rig-core) (with the OpenRouter provider) for conversational agents, and `async-openai` for direct OpenAI API access.

The project ships four binaries that share a common library (`polyrig`):

| Binary | Purpose | LLM Provider |
|--------|---------|-------------|
| `polyrig` | Interactive CLI for chat and language practice | OpenRouter |
| `polyrig-bot` | Telegram bot for LLM interaction | OpenRouter |
| `translate-subs` | CLI to translate `.srt` subtitle files | OpenRouter |
| `speech-to-text` | CLI to transcribe/translate audio | OpenAI (direct) |

```
polyrig/
├── Cargo.toml
├── conf/
│   ├── talks.toml              # Per-talk configuration
│   └── defaults.toml.template  # Bot config template
├── src/
│   ├── lib.rs                  # Library root (exports `talks`)
│   ├── talks.rs                # Core: Talk enum, Conversation, streaming
│   ├── talks/lang_practice.rs  # Lang, LangLevel enums
│   └── bin/
│       ├── speech-to-text.rs       # Audio transcription binary
│       ├── translate-subs.rs       # Subtitle translation binary
│       ├── polyrig/
│       │   ├── main.rs             # CLI binary entry point
│       │   └── view_markdown.rs    # Interactive scrollable markdown viewer
│       └── polyrig-bot/
│           ├── main.rs             # Bot entry point, config, state
│           └── telegram.rs         # Bot state machine and handlers
└── docs/
    └── architecture.md           # This file
```

---

## 2. Core Library (`polyrig`)

### 2.1 Module Layout

```
src/
├── lib.rs                 → pub mod talks;
├── talks.rs               → pub mod lang_practice;
│                            Talk enum
│                            Conversation struct
│                            stream_messages() fn
│                            Talk::get_conv()
│                            Talk::runs_on_bot() / runs_on_cli()
├── talks/
│   └── lang_practice.rs   → Lang enum (English, German, French, Spanish, Italian)
│                             LangLevel enum (Beginner, Intermediate, Advanced)
└── bin/
    ├── polyrig/
    │   ├── main.rs            → CLI: clap, rustyline, streaming, markdown view
    │   └── view_markdown.rs   → termimad scrollable viewer
    ├── speech-to-text.rs      → async-openai transcription
    ├── translate-subs.rs      → SRT translation
    └── polyrig-bot/
        ├── main.rs            → Bot entry, config, state
        └── telegram.rs        → State machine, streaming, TTS
```

### 2.2 `Talk` Enum

Defined in `talks.rs:143-160`, derives `clap::Subcommand` for CLI argument parsing, `strum` macros for string conversion, and `Display`/`EnumIter`/`EnumString` for enum traversal.

```rust
pub enum Talk {
    Generic,
    LanguagePractice { lang: Lang, level: LangLevel },
    TranslateSubs { lang: String },
}
```

Each variant maps to a key in `conf/talks.toml`. The `to_string()` value is used as the config lookup key, so the variant names must match the TOML section headers exactly (e.g., `"Generic LLM"`, `"Language Practice"`, `"Translate Subtitles"`).

**Platform availability**:
- `Generic` and `LanguagePractice` run on both CLI and Telegram bot.
- `TranslateSubs` is exclusive to the `translate-subs` binary.

### 2.3 `TalkConfig` / `TalksConfig` (TOML Parsing)

Both are private structs in `talks.rs:39-55`. `TalksConfig` wraps a `HashMap<String, TalkConfig>`, where each key is a talk name and each `TalkConfig` holds:

| Field | Type | Purpose |
|-------|------|---------|
| `system_prompt` | `String` | System prompt (supports `{lang}`, `{level}` interpolation) |
| `prefix` / `suffix` | `String` | Delimiters wrapped around user messages (e.g., `<correct_me>` tags) |
| `max_hist` | `Option<usize>` | Max conversation history length; `None` = no limit |
| `first_msg` | `Option<String>` | Optional initial message shown on startup |
| `generate_response` | `bool` | If `true`, sends a trigger message and waits for the first LLM response |
| `model` | `String` | Model identifier (e.g., `deepseek/deepseek-v4-flash:nitro`) |
| `temperature` | `Option<f64>` | Optional model temperature |
| `additional_params` | `Option<serde_json::Value>` | Extra parameters (e.g., JSON schema for strict output) |

### 2.4 `Conversation` Struct

Defined at `talks.rs:57-64`, this is the central state holder for a conversation:

```rust
pub struct Conversation {
    pub agent: Agent<openrouter::CompletionModel>,
    pub first_msg: Option<String>,
    pub presuff: (String, String),
    pub max_hist: Option<usize>,
    pub history: Vec<Message>,
    pub pending_user_msg: Option<Message>,
}
```

| Field | Purpose |
|-------|---------|
| `agent` | The rig-core `Agent` bound to the configured model and system prompt |
| `first_msg` | Optional initial message (shown to user on startup) |
| `presuff` | `(prefix, suffix)` tuple applied to user messages before sending |
| `max_hist` | Max history length for trimming |
| `history` | `Vec<rig_core::message::Message>` — the conversation history |
| `pending_user_msg` | User message held until the assistant response is fully received |

#### 2.4.1 `Conversation::get_conv()` (Factory)

Called via `Talk::get_conv()` at `talks.rs:164-237`. Flow:

1. **Load config** from `conf/talks.toml` via `toml::from_str()`.
2. **Look up** the talk-specific `TalkConfig` by the talk's `to_string()` value.
3. **Create OpenRouter client** and **build the agent** with model, temperature, and additional params.
4. **Interpolate** `{lang}`, `{level}` placeholders in `system_prompt` and `first_msg` for `LanguagePractice` and `TranslateSubs`.
5. **Set preamble** on the agent builder with the system prompt.
6. **Handle initial response**: If `generate_response` is `true`, sends an empty trigger message, streams the response, and seeds `history` with the exchange. The streamed response becomes `first_msg`.
7. **Return** a `Conversation` with the built agent and config.

#### 2.4.2 `Conversation::stream_response()`

At `talks.rs:107-130`. Prepares a streaming chat session:

1. Applies `prefix` + user message + `suffix` to form `full_message`.
2. Creates a `Message::user(full_message)`.
3. Calls `agent.stream_chat(full_message, history.clone())`.
4. Stores the user message in `pending_user_msg` (to be committed to history after the assistant response is received).

The user message is intentionally **not** added to `history` before streaming to avoid duplication — it's added by `add_assistant_response()` after the stream completes.

#### 2.4.3 `Conversation::add_assistant_response()`

At `talks.rs:133-140`. Commits the exchange:

1. Takes the `pending_user_msg` and pushes it to `history`.
2. Pushes `Message::assistant(response)` to `history`.
3. Calls `trim_history()` to enforce `max_hist`.

#### 2.4.4 `Conversation::trim_history()`

At `talks.rs:91-104.`:

1. If `max_hist` is set, removes oldest messages until `history.len() <= max_hist`.
2. Ensures history starts with a `User` message by dropping any leading non-user messages.

#### 2.4.5 `stream_messages()` — Streaming Adapter

At `talks.rs:67-87`. Converts rig-core's `StreamingResult<M>` (an async stream of `Result<MultiTurnStreamItem<M>>`) into `impl Stream<Item = Result<String>>`. It extracts text chunks from `StreamedAssistantContent::Text`, filters out `Reasoning` content (which is consumed but not forwarded), and forwards errors.

### 2.5 `Lang` and `LangLevel` Enums

Defined in `talks/lang_practice.rs`. `Lang` (English, German, French, Spanish, Italian) provides an `iso_639_1()` method returning the two-letter language code used for voice transcription. `LangLevel` (Beginner, Intermediate, Advanced) is used for system prompt interpolation.

Both derive `ValueEnum` (for clap CLI), `EnumString`/`Display`/`EnumIter` (for string conversion and iteration in the Telegram bot).

---

## 3. CLI Binary (`polyrig`)

**Source**: `src/bin/polyrig/main.rs`, `src/bin/polyrig/view_markdown.rs`

### 3.1 Flow

```
┌─────────────────────────────────────────────┐
│  clap: parse Talk subcommand                │
│  ┌───────────────────────────────────────┐  │
│  │ polyrig generic                       │  │
│  │ polyrig language-practice german      │  │
│  └───────────────────────────────────────┘  │
├─────────────────────────────────────────────┤
│  talk.get_conv() → Conversation             │
│  └─ Load talks.toml, build agent, init      │
├─────────────────────────────────────────────┤
│  Print first_msg if present                 │
├─────────────────────────────────────────────┤
│  rustyline: read multi-line input           │
│  ┌───────────────────────────────────────┐  │
│  │  while user_msg = read_msg() {        │  │
│  │    conv.stream_response(user_msg)     │  │
│  │    → stream_messages() → print raw    │  │
│  │    conv.add_assistant_response(resp)  │  │
│  │    view_markdown::show_scrolled_view  │  │
│  │    MadSkin: re-render latest response │  │
│  │  }                                    │  │
│  └───────────────────────────────────────┘  │
└─────────────────────────────────────────────┘
```

### 3.2 Input Model (`read_msg`)

At `main.rs:36-51`. Uses `rustyline::DefaultEditor` to read lines until an empty line (user presses Enter twice). Accumulates lines with `\n` separators. Returns `None` on empty input (exits the loop), `Some(msg)` otherwise.

### 3.3 Response Streaming

Each user exchange:
1. Calls `conversation.stream_response(user_msg).await` to get `StreamingResult`.
2. Wraps it with `stream_messages()` and `std::pin::pin!()`.
3. Polls the stream, printing each chunk to stdout **raw** (no markdown formatting) to avoid screen-clearing flicker during streaming.
4. Calls `conversation.add_assistant_response(full_response)` to commit the exchange.
5. Calls `view_markdown::show_scrolled_view()` to display an **interactive scrollable markdown view** of the full conversation history using `termimad` (supports `↑`/`↓`, `PgUp`/`PgDn`, `j`/`k`, mouse wheel, `q`/`Esc`).
6. After exiting the scroll view, re-renders the latest response with markdown formatting via `termimad::MadSkin`.

### 3.4 Interactive Scrollable View (`view_markdown`)

At `view_markdown.rs`. Uses `termimad`'s `MadView` with `crossterm` for an interactive viewer:

- Enters alternate screen mode for a clean display.
- Builds a markdown string from the conversation history (alternating `User`/`Assistant` labels separated by `---`).
- Starts the viewport scrolled to the latest message.
- Renders an info bar at the bottom with navigation hints.
- Supports arrow keys, page keys, and mouse wheel for scrolling.
- Cleans up: restores cursor, exits alternate screen, disables raw mode.

---

## 4. Telegram Bot (`polyrig-bot`)

**Source**: `src/bin/polyrig-bot/`

### 4.1 Architecture

The bot is built on [teloxide](https://docs.rs/teloxide), which provides a DSL for building Telegram bots with stateful dialogues. The architecture follows a **state machine pattern** where each state encapsulates the data needed for that stage of the conversation.

```
┌──────────────────────────────────────────────────────────────┐
│                        Dispatcher                            │
│  ┌──────────────────────────────────────────────────────────┐│
│  │  teloxide::Dispatcher(bot, schema(my_state))             ││
│  │  └─ dependencies: InMemStorage<State>                    ││
│  │  └─ ctrlc handler enabled                                ││
│  └──────────────────────────────────────────────────────────┘│
│                                                              │
│  ┌──────────────────────────────────────────────────────────┐│
│  │  schema() → UpdateHandler                                ││
│  │                                                          ││
│  │  command_handler:                                        ││
│  │    /help → help()                                        ││
│  │    /start → bouncer()                                    ││
│  │                                                          ││
│  │  message_handler (filtered by State):                    ││
│  │    voice message → do_talk_voice_with_dialogue()         ││
│  │    text message  → do_talk_with_dialogue()               ││
│  │                                                          ││
│  │  callback_query_handler (filtered by State):             ││
│  │    InitTalk        → init_talk()                         ││
│  │    ChooseLevel     → choose_level()                      ││
│  │    SetLevel        → set_level()                         ││
│  │    ChooseVoiceReply→ choose_voice_reply()                ││
│  └──────────────────────────────────────────────────────────┘│
│                                                              │
│  dialogue::enter::<State, InMemStorage<State>>()             │
└──────────────────────────────────────────────────────────────┘
```

### 4.2 Configuration

**Source**: `main.rs:27-47`

```rust
pub struct MyBotConfig {
    id_whitelist: HashSet<ChatId>,  // Empty list blocks all access
    transcription_model: String,    // Default: openai/gpt-4o-mini-transcribe
    tts_model: String,              // Default: google/gemini-3.1-flash-tts-preview
    tts_voice: String,              // Default: "Sulafat"
}
```

Config is loaded from `conf/defaults.toml` at startup via `get_conf()`. This file is git-ignored; users create it from `defaults.toml.template`.

### 4.3 State

**Source**: `main.rs:49-60` and `telegram.rs:51-81`

`MyState` is the shared context passed through all handlers:

```rust
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
```

`State` enum tracks the current step in the interaction flow:

```
Bouncer          ← Entry point, always runs first
  ↓
Start            ← Shown by bouncer (via select_talk)
  ↓
InitTalk         ← User selects a talk via inline keyboard
  ↓
┌─ Generic ──────────────→ DoTalk (active conversation)
│
└─ LanguagePractice ──→ choose_lang() → ChooseLevel
                          (select language)         ↓
                                               SetLevel
                                                  (select proficiency)
                                                    ↓
                                               ChooseVoiceReply
                                                  (enable TTS?)
                                                    ↓
                                               DoTalk (active conversation)
```

Each state variant holds the data it needs:
- `InitTalk`, `ChooseLevel`, `SetLevel`, `ChooseVoiceReply`: hold `prev: Option<MessageId>` for cleaning up inline keyboards.
- `DoTalk`: holds `my_state: MyState` and `talk: Talk`.

### 4.4 Dialogue System

The bot uses teloxide's `dialogue::enter()` middleware with `InMemStorage<State>` for per-user state persistence. Each Telegram user gets an independent dialogue instance that:

1. Stores the current `State` enum value.
2. Persists across multiple message/callback callbacks.
3. Automatically serializes/deserializes state.

### 4.5 Message Handling

#### 4.5.1 Text Messages (`do_talk_with_dialogue`)

At `telegram.rs:412-424`. Extracts text from the message and delegates to `do_talk()`.

#### 4.5.2 Voice Messages (`do_talk_voice_with_dialogue`)

At `telegram.rs:429-497`. Handles voice input:

1. Gets the voice file from Telegram and downloads it.
2. Creates an `openrouter::TranscriptionModel` from `my_state.transcription_model`.
3. For `LanguagePractice`, sets the target language for better transcription.
4. Spawns transcription in a background task with error handling.
5. Feeds the transcribed text into `do_talk()`.

#### 4.5.3 Core Conversation (`do_talk`)

At `telegram.rs:537-594`. The main conversation handler:

1. Creates a temporary `Conversation` from the stored state.
2. Calls `stream_response()` and passes the result to `send_stream()`.
3. After streaming completes, calls `add_assistant_response()`.
4. If `voice_reply` is enabled, calls `send_voice_reply()` in the background.
5. Updates the dialogue state with the new history.

### 4.6 Streaming to Telegram (`send_stream`)

At `telegram.rs:736-829`. This is the most complex function, handling Telegram's 4096-character message limit during live streaming:

```
┌─────────────────────────────────────────────────────────┐
│ send_stream()                                           │
│                                                         │
│  1. Create StreamState (tracks multiple messages)       │
│  2. Send initial "(...)" message (message zero)         │
│  3. For each streaming chunk:                           │
│     ├─ Append to current message content                │
│     ├─ Check if adding chunk would exceed 4000 chars    │
│     │   ├─ Yes: Finalize current, create new message    │
│     │   └─ No: Append to current                        │
│     ├─ If 2.5s since last update:                       │
│     │   └─ Update all tracked messages via edit_message │
│     └─ On error: Append error text to last message      │
│  4. Finalize: remove "(...)" suffix from last message   │
└─────────────────────────────────────────────────────────┘
```

**`StreamState`** (at `telegram.rs:677-733`) tracks multiple Telegram messages being updated simultaneously:

```rust
struct StreamState {
    messages: Vec<(MessageId, String)>,
}
```

Key methods:
- `would_exceed_limit(additional_chars)`: Checks if adding more text would exceed Telegram's 4000-char limit (with 7-char suffix buffer).
- `get_message_text(idx, is_final)`: Returns the full text for a message, adding `\n(...)` suffix if there are more messages coming.
- `update_current_content()`: Updates the last tracked message's content.

#### 4.6.1 Markdown Handling

Both `send_text_chunks()` (at `telegram.rs:597-652`) and `update_markdown()` (at `telegram.rs:655-674`) attempt to send/edit with `ParseMode::Markdown` first, then fall back to plain text if markdown parsing fails. This allows the LLM to include markdown formatting in its responses while gracefully handling unsupported syntax.

#### 4.6.2 Long Message Splitting

`send_text_chunks()` handles the initial message (and any message that exceeds 4000 chars) by:
1. Collecting characters into a `Vec<char>`.
2. Finding sentence boundaries (`.!?:?;؟。،？！؛۔` and others) within a 200-char window before the limit.
3. Splitting at the found boundary.
4. Appending `\n(...)` to intermediate chunks.

### 4.7 Voice Reply (`send_voice_reply`)

At `telegram.rs:500-534`. Generates TTS audio and sends it as a voice message:

1. Spawns TTS generation in a background task.
2. Creates an `openrouter::AudioGenerationModel` from `my_state.tts_model`.
3. Sends the audio bytes as `InputFile::memory(...)` with filename `reply.mp3`.

---

## 5. Subtitle Translator (`translate-subs`)

**Source**: `src/bin/translate-subs.rs`

### 5.1 Flow

```
┌─────────────────────────────────────────────────────┐
│ CLI Args:                                           │
│   polyrig translate-subs input.srt output.srt it    │
│     --chunk 128  (custom chunk size)                │
├─────────────────────────────────────────────────────┤
│  Translator::new(lang)                              │
│  └─ Talk::TranslateSubs.get_conv()                  │
├─────────────────────────────────────────────────────┤
│  SubRip::parse(input.srt)                           │
├─────────────────────────────────────────────────────┤
│  for chunk in chunker(subtitles, chunk_size):       │
│    Translator::translate_chunk(chunk)               │
│    ┌───────────────────────────────────────────┐    │
│    │ 1. chunk_to_json()                        │    │
│    │    → BTreeMap<random_label, text>         │    │
│    │ 2. translate_str(json_str)                │    │
│    │    → stream_messages() → full text        │    │
│    │ 3. json_to_chunk(translated_json)         │    │
│    │    → Vec<SrtSubtitle>                     │    │
│    │ 4. assemble_blocks()                      │    │
│    │    → distribute text across frames        │    │
│    └───────────────────────────────────────────┘    │
│    └─ On error: retry with fresh translator         │
│       (up to 3 times)                               │
├─────────────────────────────────────────────────────┤
│  Write translated blocks to output.srt              │
└─────────────────────────────────────────────────────┘
```

### 5.2 Chunking Strategy

The `chunker()` function (at `translate-subs.rs:312-343`) splits subtitles into groups of N blocks, preferring to split at sentence boundaries:

```rust
let win = 5; // window size to look for sentence terminator
```

For each chunk boundary, it looks backwards up to 5 subtitles for a sentence terminator (`.!?:؟。？！。♪*"`). If found, it adjusts the boundary to split there. If not, it logs a warning and uses the original boundary.

### 5.3 JSON Translation Protocol

**`chunk_to_json()`** (at `translate-subs.rs:180-191`):
1. Joins text lines within each subtitle block with spaces.
2. Creates a `BTreeMap<String, String>` where keys are `"{sequence_number}{random_5char_label}"`.
3. Returns the JSON string and the list of labels for later verification.

The random 5-character alphanumeric labels serve as anti-reordering guards — they prevent the LLM from reordering subtitle blocks while translating.

**`json_to_chunk()`** (at `translate-subs.rs:253-301`):
1. Parses the translated JSON back into a `BTreeMap<String, String>`.
2. Iterates through translated keys, matching them against the original input labels.
3. If a translated value covers multiple input blocks (because the LLM merged them), calls `assemble_blocks()`.

**`assemble_blocks()`** (at `translate-subs.rs:236-250`):
1. Calls `split_into_frames()` to distribute translated text across the original subtitle frames.
2. Returns new `SrtSubtitle` blocks with translated text but preserved timing/metadata.

**`split_into_frames()`** (at `translate-subs.rs:194-233`):
Handles the case where the LLM merges multiple subtitle lines into one translation. Three strategies:
1. **Line-level splitting**: If there are enough lines, distribute them evenly across frames.
2. **Word-level splitting**: If there are enough words, split the text by words.
3. **Repetition**: If neither works, repeat the text in each frame (last resort).

### 5.4 Error Handling & Retry

`translate_chunk()` (at `translate-subs.rs:129-163`) implements a retry mechanism:
1. Attempts translation; if parsing fails, logs the error.
2. Creates a **fresh** `Translator` (new conversation) to avoid polluting context.
3. Recurses up to 3 times before giving up and returning the original (untranslated) block verbatim.

Each translation attempt has a 90-second timeout (`timeout()` at `translate-subs.rs:97`).

---

## 6. Speech-to-Text (`speech-to-text`)

**Source**: `src/bin/speech-to-text.rs`

This binary operates independently of the core library, using `async-openai` directly.

### 6.1 Flow

```
┌─────────────────────────────────────────────────────┐
│ CLI Args:                                           │
│   speech-to-text audio.ogg output.txt               │
│   speech-to-text audio.ogg output.txt --srt         │
│   speech-to-text audio.ogg output.txt --to-eng      │
├─────────────────────────────────────────────────────┤
│  Client::new() (reads OPENAI_API_KEY)               │
├─────────────────────────────────────────────────────┤
│  if --to-eng:                                       │
│    handle_translation()                             │
│    └─ whisper-1 translation API                     │
│  else:                                              │
│    handle_transcription()                           │
│    └─ whisper-1 (SRT output) or                     │
│      gpt-4o-transcribe-diarize (JSON output)        │
├─────────────────────────────────────────────────────┤
│  Write output to file                               │
└─────────────────────────────────────────────────────┘
```

### 6.2 Output Modes

| Mode | Flag | Model | Format |
|------|------|-------|--------|
| Transcription (default) | none | `whisper-1` | SRT or JSON |
| Transcription (diarized) | none (JSON) | `gpt-4o-transcribe-diarize` | Diarized JSON |
| Translation to English | `--to-eng` | `whisper-1` | SRT or JSON |

### 6.3 Diarized JSON

When outputting JSON (default), the diarized model (`gpt-4o-transcribe-diarize`) is used, which provides speaker identification (diarization) alongside transcription. The output is pretty-printed JSON written to the output file.

---

## 7. Data Flow Summary

### 7.1 Shared Components

```
┌─────────────────────────────────────────────────────┐
│                  rig-core                           │
│  ┌────────────┐  ┌──────────────┐  ┌────────────┐   │
│  │ Agent      │  │ Streaming    │  │ Message    │   │
│  │ (OpenRouter│  │ Chat API     │  │ (text)     │   │
│  │  Provider) │  │              │  │            │   │
│  └────────────┘  └──────────────┘  └────────────┘   │
│  ┌────────────┐  ┌──────────────┐                   │
│  │ Audio      │  │ Transcription│                   │
│  │ Generation │  │ API          │                   │
│  │ (TTS)      │  │              │                   │
│  └────────────┘  └──────────────┘                   │
└─────────────────────────────────────────────────────┘
        ▲                    ▲
        │                    │
┌───────┴───────┐  ┌────────┴──────────┐
│  polyrig lib  │  │  async-openai     │
│  (Conversation│  │  (speech-to-text  │
│   + streaming)│  │   binary only)    │
└───────┬───────┘  └───────────────────┘
        │
  ┌─────┼──────────┬───────────┐
  ▼     ▼          ▼           ▼
CLI    Bot      Subs      S2T
```

### 7.2 Conversation Lifecycle (CLI)

```
Talk::get_conv()          stream_response()    add_assistant_response()
       │                        │                        │
       ▼                        ▼                        ▼
  ┌─────────┐          ┌─────────────┐          ┌─────────────┐
  │ Build   │────────▶ │ Stream      │─────────▶│ Commit to   │
  │ Agent   │          │ Response    │          │ History     │
  │ + Init  │          │ (stream)    │          │ + Trim      │
  └─────────┘          └─────────────┘          └─────────────┘
       │                        │                        │
       ▼                        ▼                        ▼
  History = []          History unchanged          History += user
                        pending_user_msg set       + assistant
```

### 7.3 Conversation Lifecycle (Telegram Bot)

```
State Machine → DoTalk → do_talk() → send_stream() → update_markdown()
     │              │           │                │
     │              │           │                ├── Print chunks
     │              │           │                ├── Split at 4000 chars
     │              │           │                └── Edit with markdown
     │              │           │
     │              │           ├── stream_response()
     │              │           │
     │              │           ├── add_assistant_response()
     │              │           │
     │              │           └── send_voice_reply() (if enabled)
     │              │
     │              └── dialogue.update(State::DoTalk)
     │                    (persist new history)
     │
     └── dialogue.update(State::...) (state transitions)
```

---

## 8. Configuration Files

### 8.1 `conf/talks.toml`

```toml
[talks."Generic LLM"]
system_prompt = "You are a helpful..."
prefix = ""
suffix = ""
max_hist = 40
first_msg = "Hey there! What's on your mind?"
generate_response = false
model = "deepseek/deepseek-v4-flash:nitro"

[talks."Language Practice"]
system_prompt = "You are PolyRig, an AI language tutor..."
prefix = "<correct_me>\n"
suffix = "\n</correct_me>"
max_hist = 100
first_msg = "Let's practice! You can now send your first message in {lang}."
generate_response = false
temperature = 0.2
model = "google/gemma-4-26b-a4b-it:nitro"

[talks."Translate Subtitles"]
system_prompt = "You are PolyRig, an expert AI for translating movie subtitles."
prefix = ""
suffix = ""
max_hist = 4
generate_response = false
temperature = 0.1
additional_params = { response_format = { type = "json_schema", ... } }
model = "google/gemini-2.5-flash-lite-preview-09-2025"
```

### 8.2 `conf/defaults.toml` (git-ignored)

```toml
id_whitelist = []           # Set to your Telegram ChatId
transcription_model = "openai/gpt-4o-mini-transcribe"
tts_model = "google/gemini-3.1-flash-tts-preview"
tts_voice = "Sulafat"
```

---

## 9. Dependencies

| Crate | Purpose | Used By |
|-------|---------|---------|
| `rig-core` | LLM abstraction (OpenRouter provider) | Core lib, CLI, Bot, Subs |
| `async-openai` | Direct OpenAI API client | S2T binary, Bot (TTS/transcription) |
| `teloxide` | Telegram bot framework | Bot |
| `clap` | CLI argument parsing (derive) | All binaries |
| `rustyline` | Readline for interactive input | CLI |
| `tokio` | Async runtime | All binaries |
| `tokio-stream` | Stream utilities | Core lib, CLI, Bot, Subs |
| `serde` / `serde_json` | Serialization | Core lib, Bot, Subs |
| `toml` | TOML config parsing | Core lib, Bot |
| `strum` / `strum_macros` | Enum string conversion | Core lib, Bot |
| `subtp` | SRT subtitle parsing | Subs |
| `chrono` | Date/time handling | Bot |
| `rand` | Random label generation | Subs |
| `anyhow` | Error handling | All binaries |
| `log` / `pretty_env_logger` / `env_logger` | Logging | Bot, Subs |
| `termimad` | Markdown rendering, interactive views | CLI |
| `async-stream` | Async stream macros | Core lib |

---

## 10. Notable Design Decisions

### 10.1 Conversation History Asymmetry

`stream_response()` adds the user message to `pending_user_msg` (not `history`), and `add_assistant_response()` commits both. If streaming fails mid-response, the user message sits in `pending_user_msg` and is not committed — preventing partial exchanges. This is intentional but means a failed stream leaves the conversation at the last complete exchange.

### 10.2 Fresh Translator on Retry

The subtitle translator creates a new `Conversation` for each retry attempt. This prevents error messages from polluting the conversation context, which could degrade translation quality on subsequent attempts.

### 10.3 Markdown Fallback

Both the bot's message editing and initial message sending try `ParseMode::Markdown` first, then fall back to plain text. This allows LLM output to include markdown formatting while gracefully handling cases where the model produces unparseable markdown syntax.

### 10.4 Random Labels in Subtitle Translation

Random 5-character alphanumeric labels in the JSON protocol prevent the LLM from reordering subtitle blocks. The labels are verified during parsing — if the LLM returns keys that don't match the input labels, an error is raised and the chunk is retried.

### 10.5 Telegram 4000-Character Limit

The `MAX_TG_CHARS` constant (4000) is respected by splitting long messages at sentence boundaries. During streaming, `StreamState` tracks multiple messages and creates new ones as the response grows, with `\n(...)` suffixes on intermediate messages that are removed on finalization.

### 10.6 Speech-to-Text Model Choice

When outputting JSON (default), the binary uses `gpt-4o-transcribe-diarize` instead of `whisper-1`. This model provides speaker diarization (identifying different speakers) in its output, which is useful for multi-speaker audio. The `--to-eng` translation path uses standard `whisper-1`.

---

## 11. File Reference Index

| File | Lines | Key Types / Functions |
|------|-------|----------------------|
| `src/lib.rs` | 17 | Module export |
| `src/talks.rs` | 256 | `Talk` enum, `Conversation` struct, `stream_messages()` |
| `src/talks/lang_practice.rs` | 48 | `Lang`, `LangLevel` enums |
| `src/bin/polyrig/main.rs` | 115 | `main()`, `read_msg()` |
| `src/bin/polyrig/view_markdown.rs` | 168 | `build_history_markdown()`, `show_scrolled_view()` |
| `src/bin/polyrig-bot/main.rs` | 111 | `main()`, `get_conf()`, `MyBotConfig`, `MyState` |
| `src/bin/polyrig-bot/telegram.rs` | 830 | `State` enum, `schema()`, `do_talk()`, `send_stream()`, `send_voice_reply()`, `StreamState` |
| `src/bin/translate-subs.rs` | 369 | `main()`, `Translator`, `chunker()`, `chunk_to_json()`, `json_to_chunk()`, `assemble_blocks()` |
| `src/bin/speech-to-text.rs` | 126 | `main()`, `handle_transcription()`, `handle_translation()` |
