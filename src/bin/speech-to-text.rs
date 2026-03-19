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
use async_openai::Client;
use async_openai::config::OpenAIConfig;
use async_openai::types::audio::{
    AudioResponseFormat, CreateTranscriptionRequestArgs, CreateTranslationRequestArgs,
    TranscriptionChunkingStrategy, TranslationResponseFormat,
};
use clap::Parser;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Input audio file (mp3, mp4, mpeg, mpga, m4a, wav, or webm)
    audio_fn: PathBuf,
    /// Output text file
    out_txt: PathBuf,
    /// The model will try to match the style of the prompt
    #[arg(long)]
    prompt: Option<String>,
    /// Produce SubRip SRT output
    #[arg(long, default_value_t = false)]
    srt: bool,
    /// Translate into English
    #[arg(long, default_value_t = false)]
    to_eng: bool,
}

async fn handle_translation(
    client: &Client<OpenAIConfig>,
    args: &Args,
    out_file: &mut File,
) -> Result<()> {
    let fmt = if args.srt {
        TranslationResponseFormat::Srt
    } else {
        TranslationResponseFormat::Json
    };

    let mut request = CreateTranslationRequestArgs::default()
        .file(args.audio_fn.clone())
        .model("whisper-1")
        .response_format(fmt)
        .build()?;
    request.prompt = args.prompt.clone();

    if args.srt {
        println!("Producing SRT translation output...");
        let response = client.audio().translation().create_raw(request).await?;
        writeln!(out_file, "{}", String::from_utf8_lossy(response.as_ref()))?;
    } else {
        println!("Producing TXT translation output...");
        let response = client.audio().translation().create(request).await?;
        writeln!(out_file, "{}", response.text)?;
    }
    Ok(())
}

async fn handle_transcription(
    client: &Client<OpenAIConfig>,
    args: &Args,
    out_file: &mut File,
) -> Result<()> {
    if args.srt {
        println!("Producing SRT transcription output...");
        let mut request = CreateTranscriptionRequestArgs::default()
            .file(args.audio_fn.clone())
            .model("whisper-1")
            .response_format(AudioResponseFormat::Srt)
            .build()?;
        request.prompt = args.prompt.clone();
        let response = client.audio().transcription().create_raw(request).await?;
        writeln!(out_file, "{}", String::from_utf8_lossy(response.as_ref()))?;
    } else {
        println!("Producing Diarized JSON transcription output...");
        let mut request = CreateTranscriptionRequestArgs::default()
            .file(args.audio_fn.clone())
            .model("gpt-4o-transcribe-diarize")
            .chunking_strategy(TranscriptionChunkingStrategy::Auto)
            .response_format(AudioResponseFormat::DiarizedJson)
            .build()?;
        request.prompt = args.prompt.clone();
        let response = client
            .audio()
            .transcription()
            .create_diarized_json(request)
            .await?;
        let json_output = serde_json::to_string_pretty(&response)?;
        writeln!(out_file, "{}", json_output)?;
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let client = Client::new();
    let mut out_file = File::create(&args.out_txt)?;

    if args.to_eng {
        handle_translation(&client, &args, &mut out_file).await?;
    } else {
        handle_transcription(&client, &args, &mut out_file).await?;
    }

    Ok(())
}
