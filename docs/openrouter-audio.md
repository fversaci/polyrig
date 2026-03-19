# Audio

How to send and receive audio with OpenRouter models

OpenRouter supports audio through three approaches:

1. **Speech-to-Text (STT)** — a dedicated `/api/v1/audio/transcriptions` endpoint for transcription
2. **Text-to-Speech (TTS)** — a dedicated `/api/v1/audio/speech` endpoint for synthesis
3. **Multimodal chat** — send audio as part of a chat request, or receive audio responses via streaming

This guide covers all three approaches.

---

## Speech-to-Text (STT)

OpenRouter supports speech-to-text (STT) via a dedicated `/api/v1/audio/transcriptions` endpoint. Send base64-encoded audio and receive a JSON response with the transcribed text and usage statistics.

### Model Discovery

You can find STT models in several ways:

**Via the API**

Use the `output_modalities` query parameter on the [Models API](/docs/api-reference/models/get-models) to discover STT models:

```bash
# List only STT models
curl "https://openrouter.ai/api/v1/models?output_modalities=transcription"
```

**On the Models Page**

Visit the [Models page](/models) and filter by output modalities to find models capable of audio transcription. You can also browse the [Speech-to-Text collection](/collections/speech-to-text-models) for a curated list.

### API Usage

Send a `POST` request to `/api/v1/audio/transcriptions` with a JSON body containing base64-encoded audio. The response is JSON with the transcribed text and optional usage statistics.

**TypeScript SDK**

```typescript
import { OpenRouter } from '@openrouter/sdk';
import fs from 'fs';

const openRouter = new OpenRouter({
  apiKey: '{{API_KEY_REF}}',
});

const audioBuffer = await fs.promises.readFile('audio.wav');
const base64Audio = audioBuffer.toString('base64');

const result = await openRouter.stt.createTranscription({
  model: '{{MODEL}}',
  inputAudio: {
    data: base64Audio,
    format: 'wav',
  },
});

console.log(result.text);
```

**Python**

```python
import requests
import base64
import json

with open("audio.wav", "rb") as f:
    base64_audio = base64.b64encode(f.read()).decode("utf-8")

response = requests.post(
    url="https://openrouter.ai/api/v1/audio/transcriptions",
    headers={
        "Authorization": "Bearer {{API_KEY_REF}}",
        "Content-Type": "application/json"
    },
    data=json.dumps({
        "model": "{{MODEL}}",
        "input_audio": {
            "data": base64_audio,
            "format": "wav"
        }
    })
)

result = response.json()
print(result["text"])
```

**TypeScript (fetch)**

```typescript
import fs from 'fs';

const audioBuffer = await fs.promises.readFile('audio.wav');
const base64Audio = audioBuffer.toString('base64');

const response = await fetch('https://openrouter.ai/api/v1/audio/transcriptions', {
  method: 'POST',
  headers: {
    Authorization: `Bearer {{API_KEY_REF}}`,
    'Content-Type': 'application/json',
  },
  body: JSON.stringify({
    model: '{{MODEL}}',
    input_audio: {
      data: base64Audio,
      format: 'wav',
    },
  }),
});

const result = await response.json();
console.log(result.text);
```

**cURL**

```bash
# Base64-encode your audio file
AUDIO_BASE64=$(base64 < audio.wav | tr -d '\n')

curl https://openrouter.ai/api/v1/audio/transcriptions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $OPENROUTER_API_KEY" \
  -d '{
    "model": "{{MODEL}}",
    "input_audio": {
      "data": "'"$AUDIO_BASE64"'",
      "format": "wav"
    }
  }'
```

### Request Parameters

| Parameter            | Type   | Required | Description                                                                           |
| -------------------- | ------ | -------- | ------------------------------------------------------------------------------------- |
| `model`              | string | Yes      | The STT model to use (e.g., `openai/whisper-1`)                                       |
| `input_audio`        | object | Yes      | Audio data to transcribe                                                              |
| `input_audio.data`   | string | Yes      | Base64-encoded audio data (raw bytes, not a data URI)                                 |
| `input_audio.format` | string | Yes      | Audio format (e.g., `wav`, `mp3`, `flac`, `m4a`, `ogg`, `webm`, `aac`)                |
| `language`           | string | No       | ISO-639-1 language code (e.g., `"en"`, `"ja"`). Auto-detected if omitted              |
| `temperature`        | number | No       | Sampling temperature between 0 and 1. Lower values produce more deterministic results |
| `provider`           | object | No       | Provider-specific passthrough configuration                                           |

### Provider-Specific Options

You can pass provider-specific options using the `provider` parameter. Options are keyed by provider slug, and only the options for the matched provider are forwarded:

```json
{
  "model": "openai/whisper-large-v3",
  "input_audio": {
    "data": "UklGRiQA...",
    "format": "wav"
  },
  "provider": {
    "options": {
      "groq": {
        "prompt": "Expected vocabulary: OpenRouter, API, transcription"
      }
    }
  }
}
```

### Response Format

The STT endpoint returns a JSON response with the transcribed text:

```json
{
  "text": "Hello, this is a test of speech-to-text transcription.",
  "usage": {
    "seconds": 9.2,
    "total_tokens": 113,
    "input_tokens": 83,
    "output_tokens": 30,
    "cost": 0.000508
  }
}
```

**Response Fields**

| Field                 | Type   | Description                                  |
| --------------------- | ------ | -------------------------------------------- |
| `text`                | string | The transcribed text                         |
| `usage.seconds`       | number | Duration of the input audio in seconds       |
| `usage.total_tokens`  | number | Total number of tokens used (input + output) |
| `usage.input_tokens`  | number | Number of input tokens billed                |
| `usage.output_tokens` | number | Number of output tokens generated            |
| `usage.cost`          | number | Total cost of the request in USD             |

**Response Headers**

| Header            | Description                                                             |
| ----------------- | ----------------------------------------------------------------------- |
| `X-Generation-Id` | Unique generation ID for the request, useful for tracking and debugging |

### Supported Audio Formats

Supported audio formats vary by provider. Common formats include:

| Format | MIME Type    | Description                              |
| ------ | ------------ | ---------------------------------------- |
| `wav`  | `audio/wav`  | Uncompressed audio, highest quality      |
| `mp3`  | `audio/mpeg` | Compressed audio, widely compatible      |
| `flac` | `audio/flac` | Lossless compressed audio                |
| `m4a`  | `audio/mp4`  | MPEG-4 audio                             |
| `ogg`  | `audio/ogg`  | Ogg Vorbis audio                         |
| `webm` | `audio/webm` | WebM audio, common in browser recordings |
| `aac`  | `audio/aac`  | Advanced Audio Coding                    |

### Pricing

STT models use different pricing strategies depending on the provider:

- **Duration-based** (e.g., OpenAI Whisper): Priced per second of audio input
- **Token-based** (e.g., newer OpenAI models): Priced per input/output token, similar to text models

You can check the cost for each model on the [Models page](/models) or via the [Models API](/docs/api-reference/models/get-models). The `usage.cost` field in the response shows the actual cost for each request.

### BYOK (Bring Your Own Key)

STT supports [BYOK](/docs/guides/overview/auth/byok), allowing you to use your own provider API keys. When configured, requests are routed directly to the provider using your key, and OpenRouter charges only its platform fee rather than the per-usage model cost.

### Playground

You can test STT models directly in the browser using the [OpenRouter Playground](/playground). Navigate to any STT model's page and use the playground tab to upload an audio file and see the transcription result.

### Differences from Audio Input

OpenRouter supports two ways to process audio:

1. **Speech-to-Text** (this section): A dedicated `/api/v1/audio/transcriptions` endpoint optimized for transcription. Returns structured JSON with the transcribed text and usage data. Best for converting audio to text.

2. **Audio input via Chat Completions** ([Audio docs](/docs/features/multimodal/audio)): Send audio as part of a `/api/v1/chat/completions` request using the `input_audio` content type. The model processes the audio alongside text and responds conversationally. Best for audio analysis, question answering about audio content, or combining audio with other modalities.

### Best Practices

- **Choose the right format**: WAV provides the best quality for transcription. MP3 and other compressed formats work well but may slightly reduce accuracy for borderline audio
- **File size**: For very long audio files, consider splitting them into smaller segments. The upstream provider timeout is 60 seconds, so very large files may time out
- **Base64 encoding**: Audio must be sent as base64-encoded data (raw bytes, not a data URI). Most programming languages have built-in base64 encoding utilities

### Troubleshooting

**Empty or incorrect transcription?**

- Verify the audio format matches the `format` field in your request
- Ensure the audio quality is sufficient for transcription

**Request timing out?**

- Large audio files may exceed the 60-second timeout. Split long recordings into smaller segments
- Compressed formats (MP3, AAC) produce smaller payloads and transfer faster

**Model not found?**

- Use the [Models page](/models) or the [Models API](/docs/api-reference/models/get-models) with `output_modalities=transcription` to find available STT models
- Verify the model slug is correct (e.g., `openai/whisper-1`, not `whisper-1`)

**Authentication error?**

- Ensure you're using a valid API key from [your OpenRouter dashboard](/settings/keys)
- The STT endpoint uses the same authentication as the Chat Completions API

---

## Text-to-Speech (TTS)

OpenRouter supports text-to-speech (TTS) via a dedicated `/api/v1/audio/speech` endpoint that is compatible with the [OpenAI Audio Speech API](https://platform.openai.com/docs/api-reference/audio/createSpeech). Send text and receive a raw audio byte stream in your chosen format.

### Model Discovery

You can find TTS models in several ways:

**Via the API**

Use the `output_modalities` query parameter on the [Models API](/docs/api-reference/models/get-models) to discover TTS models:

```bash
# List only TTS models
curl "https://openrouter.ai/api/v1/models?output_modalities=speech"
```

**On the Models Page**

Visit the [Models page](/models) and filter by output modalities to find models capable of speech synthesis. Look for models that list `"speech"` in their output modalities.

### API Usage

Send a `POST` request to `/api/v1/audio/speech` with the text you want to synthesize. The response is a raw audio byte stream — not JSON — so you can pipe it directly to a file or audio player.

**TypeScript SDK**

```typescript
import { OpenRouter } from '@openrouter/sdk';
import fs from 'fs';

const openRouter = new OpenRouter({
  apiKey: '{{API_KEY_REF}}',
});

const stream = await openRouter.tts.createSpeech({
  model: '{{MODEL}}',
  input: 'Hello! This is a text-to-speech test.',
  voice: 'alloy',
  responseFormat: 'mp3',
});

// Collect the audio stream and save to a file
const reader = stream.getReader();
const chunks: Uint8Array[] = [];
while (true) {
  const { done, value } = await reader.read();
  if (done) break;
  chunks.push(value);
}
const totalLength = chunks.reduce((sum, c) => sum + c.length, 0);
const buffer = new Uint8Array(totalLength);
let offset = 0;
for (const chunk of chunks) {
  buffer.set(chunk, offset);
  offset += chunk.length;
}
await fs.promises.writeFile('output.mp3', buffer);
console.log('Audio saved to output.mp3');
```

**OpenAI Python**

```python
from openai import OpenAI

client = OpenAI(
  base_url="https://openrouter.ai/api/v1",
  api_key="{{API_KEY_REF}}",
)

with client.audio.speech.with_streaming_response.create(
  model="{{MODEL}}",
  input="Hello! This is a text-to-speech test.",
  voice="alloy",
  response_format="mp3"
) as response:
  response.stream_to_file("output.mp3")
```

**Python (requests)**

```python
import requests

response = requests.post(
  url="https://openrouter.ai/api/v1/audio/speech",
  headers={
    "Authorization": f"Bearer {API_KEY_REF}",
    "Content-Type": "application/json"
  },
  json={
    "model": "{{MODEL}}",
    "input": "Hello! This is a text-to-speech test.",
    "voice": "alloy",
    "response_format": "mp3"
  }
)
response.raise_for_status()

with open("output.mp3", "wb") as f:
  f.write(response.content)

generation_id = response.headers.get("X-Generation-Id")
print(f"Audio saved. Generation ID: {generation_id}")
```

**TypeScript (fetch)**

```typescript
const response = await fetch('https://openrouter.ai/api/v1/audio/speech', {
  method: 'POST',
  headers: {
    Authorization: `Bearer ${API_KEY_REF}`,
    'Content-Type': 'application/json',
  },
  body: JSON.stringify({
    model: '{{MODEL}}',
    input: 'Hello! This is a text-to-speech test.',
    voice: 'alloy',
    response_format: 'mp3',
  }),
});

if (!response.ok) {
  const err = await response.json();
  throw new Error(`TTS error ${response.status}: ${JSON.stringify(err)}`);
}

const audioBuffer = await response.arrayBuffer();
const generationId = response.headers.get('X-Generation-Id');
console.log(`Generation ID: ${generationId}`);
// Save audioBuffer to a file or play it directly
```

**cURL**

```bash
curl https://openrouter.ai/api/v1/audio/speech \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $OPENROUTER_API_KEY" \
  --output output.mp3 \
  -d '{
    "model": "{{MODEL}}",
    "input": "Hello! This is a text-to-speech test.",
    "voice": "alloy",
    "response_format": "mp3"
  }'
```

### Request Parameters

| Parameter         | Type   | Required | Description                                                                                                                      |
| ----------------- | ------ | -------- | -------------------------------------------------------------------------------------------------------------------------------- |
| `model`           | string | Yes      | The TTS model to use (e.g., `openai/gpt-4o-mini-tts-2025-12-15`, `mistralai/voxtral-mini-tts-2603`)                              |
| `input`           | string | Yes      | The text to synthesize into speech                                                                                               |
| `voice`           | string | Yes      | Voice identifier. Available voices vary by model — check each model's page on the [Models page](/models) for supported voices    |
| `response_format` | string | No       | Audio output format: `mp3` or `pcm`. Defaults to `pcm`                                                                           |
| `speed`           | number | No       | Playback speed multiplier. Only used by models that support it (e.g., OpenAI TTS). Ignored by other providers. Defaults to `1.0` |
| `provider`        | object | No       | Provider-specific passthrough configuration                                                                                      |

### Provider-Specific Options

You can pass provider-specific options using the `provider` parameter. Options are keyed by provider slug, and only the options for the matched provider are forwarded:

```json
{
  "model": "openai/gpt-4o-mini-tts-2025-12-15",
  "input": "Hello world",
  "voice": "alloy",
  "provider": {
    "options": {
      "openai": {
        "instructions": "Speak in a warm, friendly tone."
      }
    }
  }
}
```

### Response Format

The TTS endpoint returns a **raw audio byte stream**, not JSON. The response includes the following headers:

| Header            | Description                                                                             |
| ----------------- | --------------------------------------------------------------------------------------- |
| `Content-Type`    | The MIME type of the audio. `audio/mpeg` for `mp3` format, `audio/pcm` for `pcm` format |
| `X-Generation-Id` | The unique generation ID for the request, useful for tracking and debugging             |

**Output Formats**

| Format | Content-Type | Description                                                                       |
| ------ | ------------ | --------------------------------------------------------------------------------- |
| `mp3`  | `audio/mpeg` | Compressed audio, smaller file size. Good for storage and playback                |
| `pcm`  | `audio/pcm`  | Uncompressed raw audio. Lower latency, suitable for real-time streaming pipelines |

### Pricing

TTS models are priced **per character** of input text. Pricing varies by model and provider. You can check the per-character cost for each model on the [Models page](/models) or via the [Models API](/docs/api-reference/models/get-models).

### OpenAI SDK Compatibility

The TTS endpoint is fully compatible with the OpenAI SDK. You can use the OpenAI client libraries by pointing them at OpenRouter's base URL:

**OpenAI Python SDK**

```python
from openai import OpenAI

client = OpenAI(
  base_url="https://openrouter.ai/api/v1",
  api_key="{{API_KEY_REF}}",
)

# Non-streaming: get the full audio response
response = client.audio.speech.create(
  model="openai/gpt-4o-mini-tts-2025-12-15",
  input="The quick brown fox jumps over the lazy dog.",
  voice="nova",
  response_format="mp3"
)
response.write_to_file("output.mp3")

# Streaming: process audio chunks as they arrive
with client.audio.speech.with_streaming_response.create(
  model="openai/gpt-4o-mini-tts-2025-12-15",
  input="The quick brown fox jumps over the lazy dog.",
  voice="nova",
  response_format="mp3"
) as response:
  response.stream_to_file("output.mp3")
```

**OpenAI TypeScript SDK**

```typescript
import OpenAI from 'openai';
import fs from 'fs';

const client = new OpenAI({
  baseURL: 'https://openrouter.ai/api/v1',
  apiKey: '{{API_KEY_REF}}',
});

const response = await client.audio.speech.create({
  model: 'openai/gpt-4o-mini-tts-2025-12-15',
  input: 'The quick brown fox jumps over the lazy dog.',
  voice: 'nova',
  response_format: 'mp3',
});

const buffer = Buffer.from(await response.arrayBuffer());
await fs.promises.writeFile('output.mp3', buffer);
console.log('Audio saved to output.mp3');
```

### Best Practices

- **Choose the right format**: Use `mp3` for storage and general playback. Use `pcm` for real-time streaming pipelines where latency matters
- **Voice selection**: Different providers offer different voices. Check the model's documentation or experiment with available voices to find the best fit for your use case
- **Input length**: For very long texts, consider splitting the input into smaller segments and concatenating the audio output. This can improve reliability and reduce latency for the first audio chunk
- **Speed parameter**: The `speed` parameter is only supported by certain providers (e.g., OpenAI). It is silently ignored by providers that don't support it

### Troubleshooting

**Empty or corrupted audio file?**

- Verify the `response_format` matches how you're saving the file (e.g., don't save `pcm` output with a `.mp3` extension)
- Check the response status code — non-200 responses return JSON error bodies, not audio

**Model not found?**

- Use the [Models page](/models) to find available TTS models
- Verify the model slug is correct (e.g., `openai/gpt-4o-mini-tts-2025-12-15`, not `gpt-4o-mini-tts`)

**Voice not available?**

- Available voices vary by provider. Check the provider's documentation for supported voice identifiers
- Each model has its own set of voices — check the model's page on the [Models page](/models) for the full list

---

## Multimodal Chat: Audio Input and Output

OpenRouter also supports sending audio files to compatible models and receiving audio responses via the Chat Completions API (`/api/v1/chat/completions`). This is useful when you need conversational interaction with audio alongside text.

### Audio Input

Send audio files to compatible models for transcription, analysis, and processing. Audio input requests use the `/api/v1/chat/completions` API with the `input_audio` content type. Audio files must be base64-encoded and include the format specification.

**Note**: Audio files must be **base64-encoded** — direct URLs are not supported for audio content.

You can search for models that support audio input by filtering to audio input modality on our [Models page](/models?fmt=cards&input_modalities=audio).

**Sending Audio Files**

**TypeScript SDK**

```typescript
import { OpenRouter } from '@openrouter/sdk';
import fs from "fs/promises";

const openRouter = new OpenRouter({
  apiKey: '{{API_KEY_REF}}',
});

async function encodeAudioToBase64(audioPath: string): Promise<string> {
  const audioBuffer = await fs.readFile(audioPath);
  return audioBuffer.toString("base64");
}

const audioPath = "path/to/your/audio.wav";
const base64Audio = await encodeAudioToBase64(audioPath);

const result = await openRouter.chat.send({
  model: "{{MODEL}}",
  messages: [
    {
      role: "user",
      content: [
        {
          type: "text",
          text: "Please transcribe this audio file.",
        },
        {
          type: "input_audio",
          inputAudio: {
            data: base64Audio,
            format: "wav",
          },
        },
      ],
    },
  ],
  stream: false,
});

console.log(result);
```

**Python**

```python
import requests
import json
import base64

url = "https://openrouter.ai/api/v1/chat/completions"
headers = {
    "Authorization": f"Bearer {API_KEY_REF}",
    "Content-Type": "application/json"
}

with open("path/to/your/audio.wav", "rb") as audio_file:
    audio_data = base64.b64encode(audio_file.read()).decode("utf-8")

payload = {
    "model": "{{MODEL}}",
    "messages": [
        {
            "role": "user",
            "content": [
                {
                    "type": "text",
                    "text": "Please transcribe this audio file."
                },
                {
                    "type": "input_audio",
                    "input_audio": {
                        "data": audio_data,
                        "format": "wav"
                    }
                }
            ]
        }
    ]
}

response = requests.post(url, headers=headers, json=payload)
print(response.json())
```

**TypeScript (fetch)**

```typescript
import fs from "fs/promises";

async function encodeAudioToBase64(audioPath: string): Promise<string> {
  const audioBuffer = await fs.readFile(audioPath);
  return audioBuffer.toString("base64");
}

const audioPath = "path/to/your/audio.wav";
const base64Audio = await encodeAudioToBase64(audioPath);

const response = await fetch("https://openrouter.ai/api/v1/chat/completions", {
  method: "POST",
  headers: {
    Authorization: `Bearer ${API_KEY_REF}`,
    "Content-Type": "application/json",
  },
  body: JSON.stringify({
    model: "{{MODEL}}",
    messages: [
      {
        role: "user",
        content: [
          {
            type: "text",
            text: "Please transcribe this audio file.",
          },
          {
            type: "input_audio",
            input_audio: {
              data: base64Audio,
              format: "wav",
            },
          },
        ],
      },
    ],
  }),
});

const data = await response.json();
console.log(data);
```

### Audio Output

OpenRouter supports receiving audio responses from models that have audio output capabilities. To request audio output, include the `modalities` and `audio` parameters in your request.

You can search for models that support audio output by filtering to audio output modality on our [Models page](/models?fmt=cards&output_modalities=audio).

**Requesting Audio Output**

To receive audio output, set `modalities` to `["text", "audio"]` and provide the `audio` configuration with your desired voice and format:

**Python**

```python
import requests
import json
import base64

url = "https://openrouter.ai/api/v1/chat/completions"
headers = {
    "Authorization": f"Bearer {API_KEY_REF}",
    "Content-Type": "application/json"
}

payload = {
    "model": "{{MODEL}}",
    "messages": [
        {
            "role": "user",
            "content": "Say hello in a friendly tone."
        }
    ],
    "modalities": ["text", "audio"],
    "audio": {
        "voice": "alloy",
        "format": "wav"
    },
    "stream": True
}

# Audio output requires streaming — the response is delivered as SSE chunks
response = requests.post(url, headers=headers, json=payload, stream=True)

audio_data_chunks = []
transcript_chunks = []

for line in response.iter_lines():
    if not line:
        continue
    decoded = line.decode("utf-8")
    if not decoded.startswith("data: "):
        continue
    data = decoded[len("data: "):]
    if data.strip() == "[DONE]":
        break
    chunk = json.loads(data)
    delta = chunk["choices"][0].get("delta", {})
    audio = delta.get("audio", {})
    if audio.get("data"):
        audio_data_chunks.append(audio["data"])
    if audio.get("transcript"):
        transcript_chunks.append(audio["transcript"])

transcript = "".join(transcript_chunks)
print(f"Transcript: {transcript}")

# Combine and decode the base64 audio chunks, then save
full_audio_b64 = "".join(audio_data_chunks)
audio_bytes = base64.b64decode(full_audio_b64)
with open("output.wav", "wb") as f:
    f.write(audio_bytes)
```

**TypeScript (fetch)**

```typescript
const response = await fetch("https://openrouter.ai/api/v1/chat/completions", {
  method: "POST",
  headers: {
    Authorization: `Bearer ${API_KEY_REF}`,
    "Content-Type": "application/json",
  },
  body: JSON.stringify({
    model: "{{MODEL}}",
    messages: [
      {
        role: "user",
        content: "Say hello in a friendly tone.",
      },
    ],
    modalities: ["text", "audio"],
    audio: {
      voice: "alloy",
      format: "wav",
    },
    stream: true,
  }),
});

// Audio output requires streaming — parse the SSE chunks
const reader = response.body!.getReader();
const decoder = new TextDecoder();

const audioDataChunks: string[] = [];
const transcriptChunks: string[] = [];
let buffer = "";

while (true) {
  const { done, value } = await reader.read();
  if (done) break;

  buffer += decoder.decode(value, { stream: true });
  const lines = buffer.split("\n");
  buffer = lines.pop()!; // keep incomplete line in buffer

  for (const line of lines) {
    if (!line.startsWith("data: ")) continue;
    const data = line.slice("data: ".length).trim();
    if (data === "[DONE]") break;

    const chunk = JSON.parse(data);
    const audio = chunk.choices?.[0]?.delta?.audio;
    if (audio?.data) audioDataChunks.push(audio.data);
    if (audio?.transcript) transcriptChunks.push(audio.transcript);
  }
}

const transcript = transcriptChunks.join("");
console.log(`Transcript: ${transcript}`);

// audioDataChunks joined together is the full base64-encoded audio
const fullAudioB64 = audioDataChunks.join("");
```

**Streaming Chunk Format**

Audio output requires streaming (`stream: true`). Audio data and transcript are delivered incrementally via the `delta.audio` field in each chunk:

```json
{
  "choices": [
    {
      "delta": {
        "audio": {
          "data": "<base64-encoded audio chunk>",
          "transcript": "Hello"
        }
      }
    }
  ]
}
```

**Audio Configuration Options**

The `audio` parameter accepts the following options:

| Option   | Description                                                                                      |
|----------|--------------------------------------------------------------------------------------------------|
| `voice`  | The voice to use for audio generation (e.g., `alloy`, `echo`, `fable`, `onyx`, `nova`, `shimmer`). Available voices vary by model. |
| `format` | The audio format for the output (e.g., `wav`, `mp3`, `flac`, `opus`, `pcm16`). Available formats vary by model. |

---

## Choosing the Right Approach

| Use case | Recommended endpoint |
|----------|---------------------|
| Convert speech to text | Dedicated STT endpoint (`/api/v1/audio/transcriptions`) |
| Generate speech from text | Dedicated TTS endpoint (`/api/v1/audio/speech`) |
| Analyze or answer questions about audio content | Chat Completions with `input_audio` content type |
| Get a conversational audio response from a model | Chat Completions with `modalities: ["text", "audio"]` |
| Combine audio with other modalities (images, PDFs) | Chat Completions with multimodal `content` array |
