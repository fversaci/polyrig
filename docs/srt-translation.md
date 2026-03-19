# SRT Translation Algorithm

This document details the algorithm used in `src/bin/translate-subs.rs` for translating SubRip (.srt) subtitle files using a Large Language Model (LLM).

## Overview

The process involves parsing the SRT file, splitting it into manageable chunks, translating each chunk via an LLM using a JSON-based protocol, and handling the response—specifically dealing with cases where the LLM might merge consecutive subtitles.

## 1. Preparation and Chunking

### Parsing (`get_parser`)
The input file is read and parsed into a `SubRip` object containing a list of `SrtSubtitle` blocks. Each block contains a sequence number, timecode, and text lines.

### Intelligent Chunking (`chunker`)
Instead of blindly splitting the subtitle list into fixed-size blocks, the algorithm attempts to preserve sentence integrity.
1.  **Target Size**: The user specifies a target chunk size (default: 64 blocks).
2.  **Sentence Boundary Detection**: The algorithm looks at a window (default: 5 blocks) at the end of the current chunk.
3.  **EOS Check**: It checks if any block in this window ends with a sentence terminator (`.`, `?`, `!`, `;`, `:`, `؟`, `。`, `？`, `！`, `।`, `♪`, `*`, `"`, `>`).
4.  **Splitting**: If a terminator is found, the chunk is split at that point. This ensures the LLM receives complete thoughts, improving translation quality.

## 2. The Core Translation Loop (`translate_chunk`)

This function manages the translation of a single slice of subtitles. It acts as a stateful orchestrator handling retries and error recovery.

### Workflow
1.  **Serialization**: The subtitle slice is converted into a JSON prompt via `chunk_to_json`.
2.  **Translation**: The JSON string is sent to the LLM via `translate_str`.
3.  **Deserialization**: The returned JSON is parsed back into subtitle blocks via `json_to_chunk`.
4.  **Error Handling**:
    - If parsing or validation fails, an error counter is incremented.
    - **Retry Logic**: If errors are below the threshold (3), the current `Translator` is replaced with a fresh one (to reset the conversation context) and the function retries the chunk via `*self = new_trans` followed by a pinned recursive call.
    - **Fallback**: If errors exceed the threshold (3), the function gives up and returns the original, untranslated subtitle blocks to ensure the output file remains complete.

## 3. Input Formatting (`chunk_to_json`)

To translate effectively, subtitles must be presented in a format the LLM understands, while allowing the code to map responses back to specific timecodes.

### Labeling Strategy
- **Random Labels**: Each subtitle block is assigned a unique key consisting of a 4-digit zero-padded index and a random 5-character alphanumeric string (e.g., `0000aB3xZ`).
- **Purpose**: The randomness prevents the LLM from hallucinating sequence patterns and ensures strict mapping between input and output keys.

### JSON Structure
The data is serialized into a JSON object:
```json
{
  "0000aB3xZ": "Hello,",
  "0001xY7qP": "how are you?"
}
```
A system prompt instructs the LLM to translate the values into the target language while preserving the keys.

## 4. Output Processing (`json_to_chunk`)

This is the most complex part of the algorithm, responsible for converting the LLM's JSON response back into `SrtSubtitle` objects.

### Handling Merged Entries
The system prompt explicitly allows the LLM to merge consecutive entries (e.g., merging "Hello," and "how are you?" into a single line). The code handles this via a "Peek and Seek" logic:

1.  **Iteration**: It iterates through the translated JSON entries.
2.  **Label Matching**: It checks if the current translated key matches the expected input key.
3.  **Merge Detection**: If the keys match, it looks ahead (`peek`) in the translated response to find the *next* key. It then searches the input list to see how many input blocks exist between the current key and the next key.
    - If the next translated key corresponds to the *next* input key, no merge occurred.
    - If the next translated key corresponds to an input key further down the list, the intermediate input blocks were merged into the current translation.
4.  **Block Assembly**: It calculates how many input blocks correspond to the single translated text and calls `assemble_blocks`.

*Note: If the LLM drops the *first* key of a sequence (which violates the prompt instructions), the function returns an error, triggering the retry logic in `translate_chunk`.*

## 5. Text Distribution (`assemble_blocks` & `split_into_frames`)

When multiple input blocks are merged into one translated text, that text must be distributed back across the original timecodes. `split_into_frames` uses a fallback hierarchy to distribute text:

1.  **Line Distribution**: If the translated text has enough lines (equal to or more than the target frames), it distributes the lines one-to-one.
2.  **Word Distribution**: If there aren't enough lines but enough words, it splits the text by words and distributes chunks of words.
3.  **Repetition**: If there are fewer words than frames, it repeats the entire text in every frame (a rare edge case).

## 6. LLM Interaction (`translate_str`)

This function manages the conversation with the LLM:
- It streams the response to minimize latency, with a 90-second timeout.
- It accumulates the full response text.
- **History Management**: It appends the user prompt and the assistant's response to the conversation history. This allows the LLM to maintain context across chunks within a single translation session, improving consistency in terminology and style. On retry (after parse/validation errors), a fresh `Translator` is created, resetting the conversation context.

## 7. Main Execution Flow

The entry point in `main()` orchestrates the entire process sequentially:
1.  **Initialization**: It parses command-line arguments (input/output files, language, chunk size).
2.  **Agent Creation**: It creates a single `Translator` instance. This instance holds the conversation state (`Conversation`) for the duration of the translation session.
3.  **Sequential Processing**: It iterates through the chunks provided by `chunker`. Each chunk is processed by `translator.translate_chunk()` one after another.
4.  **Output**: Translated blocks are written to the output file immediately after each chunk is processed.
