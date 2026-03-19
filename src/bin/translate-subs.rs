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

use anyhow::{Result, anyhow};
use clap::Parser;
use log::debug;
use polyrig::talks::{Conversation, Talk, stream_messages};
use rand::RngExt;
use rand::rngs::ThreadRng;
use std::collections::BTreeMap;
use std::fs::File;
use std::io::Write;
use std::io::{BufReader, Read};
use std::path::PathBuf;
use std::time::Duration;
use subtp::srt::{SrtSubtitle, SubRip};
use tokio::time::timeout;
use tokio_stream::StreamExt;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Input subtitle file, must be SRT
    in_srt: PathBuf,
    /// Output subtitle SRT file
    out_srt: PathBuf,
    /// Language to translate to
    lang: String,
    /// Number of blocks per query
    #[arg(long, default_value_t = 64)]
    chunk: usize,
}

struct RandLabel {
    rng: ThreadRng,
}

impl RandLabel {
    /// Creates a new random label generator.
    fn new() -> Self {
        let rng = rand::rng();
        RandLabel { rng }
    }
    /// Generates a random 5-character alphanumeric label.
    fn get_label(&mut self) -> String {
        let charset: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";

        let random_string: String = (0..5)
            .map(|_| {
                let idx = self.rng.random_range(0..charset.len());
                charset[idx] as char
            })
            .collect();

        random_string
    }
}

struct Translator {
    conv: Conversation,
    lang: String,
    rand: RandLabel,
}

impl Translator {
    /// Creates a new translator agent for the specified language.
    async fn new(lang: String) -> Result<Self> {
        let talk = Talk::TranslateSubs { lang: lang.clone() };
        let conv = talk.get_conv().await?;
        Ok(Self {
            conv,
            lang,
            rand: RandLabel::new(),
        })
    }

    /// Translates a string using the conversation agent.
    async fn translate_str(&mut self, msg: &str) -> Result<String> {
        let preview = msg.lines().take(20).collect::<Vec<_>>().join("\n");
        debug!("Sending to OpenRouter (first 20 lines):\n{}", preview);

        // Wrap the streaming operation with a 90-second timeout
        let stream_future = self.conv.stream_response(msg.to_string());
        let stream = match timeout(Duration::from_secs(90), stream_future).await {
            Ok(stream) => stream,
            Err(_) => {
                debug!("Timeout after 90s waiting for response from OpenRouter");
                return Err(anyhow!(
                    "Timeout after 90s waiting for response from OpenRouter"
                ));
            }
        };

        let mut response_text = String::new();
        let mut msg_stream = std::pin::pin!(stream_messages(stream));
        while let Some(chunk) = msg_stream.next().await {
            match chunk {
                Ok(txt) => {
                    debug!("Received chunk: {}", txt);
                    response_text.push_str(&txt);
                }
                Err(e) => return Err(e),
            }
        }
        let preview = response_text
            .lines()
            .take(20)
            .collect::<Vec<_>>()
            .join("\n");
        debug!("Full response (first 20 lines):\n{}", preview);
        self.conv.add_assistant_response(response_text.clone());
        Ok(response_text)
    }

    /// Translates a chunk of subtitles, handling errors and retries.
    async fn translate_chunk(
        &mut self,
        chunk: &[SrtSubtitle],
        mut errors: u8,
    ) -> Result<Vec<SrtSubtitle>> {
        const MAX_ERRORS: u8 = 3;
        // try and translate it
        let (in_labs, json_str) = chunk_to_json(&mut self.rand, chunk)?;
        let trans_json_str = self.translate_str(&json_str).await?;
        let ret = json_to_chunk(&trans_json_str, in_labs, chunk);
        if ret.is_ok() {
            return ret;
        }
        // Something went wrong, log error
        errors += 1;
        println!(
            "Error detected {}/{}: {}",
            errors,
            MAX_ERRORS,
            ret.err().unwrap()
        );
        // if too many errors, give up and use the original text
        if errors > MAX_ERRORS {
            println!(
                "- Giving up, copying verbatim block {}",
                chunk.first().unwrap().sequence
            );
            return Ok(chunk.to_vec());
        }
        // otherwise use a new translator and try again
        let new_trans = Translator::new(self.lang.clone()).await?;
        *self = new_trans;
        Box::pin(self.translate_chunk(chunk, errors)).await
    }
}

/// Parses an Srt file into a SubRip object.
fn get_parser(subs_fn: PathBuf) -> Result<SubRip> {
    let ext = subs_fn.extension().unwrap_or_default();
    if ext != "srt" {
        return Err(anyhow!("Subtitles filename must end in srt."));
    }
    // read subs file
    let subs_f = File::open(subs_fn)?;
    let mut subs = String::new();
    BufReader::new(subs_f).read_to_string(&mut subs)?;
    // parse text
    Ok(SubRip::parse(&subs)?)
}

/// Converts a subtitle chunk into a JSON string for translation.
fn chunk_to_json(rand: &mut RandLabel, chunk: &[SrtSubtitle]) -> Result<(Vec<String>, String)> {
    // Join text lines with a space to create a single string per subtitle block
    let chunk_text: Vec<String> = chunk.iter().map(|sub| sub.text.join(" ")).collect();
    let chunk_dict: BTreeMap<String, String> = chunk_text
        .into_iter()
        .enumerate()
        .map(|(a, b)| (format!("{:04}{}", a, rand.get_label()), b))
        .collect();
    let chunk_labs: Vec<String> = chunk_dict.keys().cloned().collect();
    let json_str = serde_json::to_string_pretty(&chunk_dict)?;
    Ok((chunk_labs, json_str))
}

/// Distributes translated text across a specified number of frames.
fn split_into_frames(trans_text: &[String], num_frames: usize) -> Vec<Vec<String>> {
    // single frame, return text as vector
    if num_frames == 1 {
        return vec![trans_text.to_owned()];
    }
    // multiple frames, enough lines, split them
    println!("Spreading text: {:?} to {} frames", trans_text, num_frames);
    let num_lines = trans_text.len();
    if num_lines >= num_frames {
        let mut result = Vec::new();
        for i in 0..num_frames {
            let start = (i * num_lines).div_ceil(num_frames);
            let end = ((i + 1) * num_lines).div_ceil(num_frames);
            let slice = &trans_text[start..end];
            result.push(slice.to_vec());
        }
        return result;
    }
    // not enough lines, split words
    let joined: String = trans_text.join(" ");
    let words: Vec<&str> = joined.split_whitespace().collect();
    let num_words = words.len();
    if num_words >= num_frames {
        let mut result = Vec::new();
        for i in 0..num_frames {
            let start = (i * num_words).div_ceil(num_frames);
            let end = ((i + 1) * num_words).div_ceil(num_frames);
            let slice = &words[start..end];
            let slice = slice.join(" ");
            result.push(vec![slice]);
        }
        return result;
    }
    // not enough words, repeat text in each frame
    let mut result = Vec::new();
    for _ in 0..num_frames {
        result.push(trans_text.to_vec());
    }
    result
}

/// Assembles translated text into subtitle blocks.
fn assemble_blocks(in_blocks: &[SrtSubtitle], trans_text: &[String]) -> Result<Vec<SrtSubtitle>> {
    let num_frames = in_blocks.len();
    let max_spread = 3;
    if num_frames > max_spread {
        return Err(anyhow!("Spreading text too much."));
    }
    let frames = split_into_frames(trans_text, num_frames);
    assert!(frames.len() == in_blocks.len());
    let mut out_blocks = Vec::new();
    for (in_block, text) in in_blocks.iter().zip(frames) {
        let trans_block = SrtSubtitle { text, ..*in_block };
        out_blocks.push(trans_block);
    }
    Ok(out_blocks)
}

/// Parses translated JSON back into subtitle blocks.
fn json_to_chunk(
    json_str: &str,
    in_labs: Vec<String>,
    in_chunk: &[SrtSubtitle],
) -> Result<Vec<SrtSubtitle>> {
    // Expect a JSON object with string values
    let trans_dict: BTreeMap<String, String> = serde_json::from_str(json_str)?;
    let mut in_curr = 0;
    let mut trans_iter = trans_dict.into_iter().peekable();
    let mut out_chunk = Vec::new();
    while let Some((trans_label, trans_data)) = trans_iter.next() {
        let in_label = in_labs.get(in_curr);
        if in_label.is_none() {
            return Err(anyhow!("Exhausted input."));
        }
        let in_label = in_label.unwrap();
        if *in_label != trans_label {
            return Err(anyhow!("Missing key label in translated chunk."));
        }
        let trans_blocks;
        if trans_iter.peek().is_none() {
            // Wrap the single string in a slice for assemble_blocks
            trans_blocks =
                assemble_blocks(&in_chunk[in_curr..], std::slice::from_ref(&trans_data))?;
        } else {
            let next_label = &trans_iter.peek().unwrap().0;
            let mut num_blocks = 0;
            let mut found = false;
            for in_lab in &in_labs[in_curr..] {
                if in_lab == next_label {
                    found = true;
                    break;
                }
                num_blocks += 1;
            }
            if found {
                trans_blocks = assemble_blocks(
                    &in_chunk[in_curr..in_curr + num_blocks],
                    std::slice::from_ref(&trans_data),
                )?;
                in_curr += num_blocks;
            } else {
                return Err(anyhow!("Next label not found."));
            }
        }
        out_chunk.extend(trans_blocks);
    }
    Ok(out_chunk)
}

/// Checks if a character is a sentence terminator.
fn is_end_of_sentence(character: &char) -> bool {
    let sentence_terminators = &[
        '.', '!', '?', ';', ':', '؟', '。', '？', '！', '।', '♪', '*', '"', '>',
    ];
    sentence_terminators.contains(character)
}

/// Splits subtitles into chunks, preferring sentence boundaries.
fn chunker(subs: &[SrtSubtitle], chunk: usize) -> impl Iterator<Item = &[SrtSubtitle]> {
    let win = 5; // window size to look for eos
    let mut ret = Vec::new();
    let mut back = 0usize;
    for (i, c) in subs.chunks(chunk).enumerate() {
        let chunk_beg = i * chunk;
        let chunk_end = chunk_beg + c.len();
        let beg = i * chunk - back;
        let mut end = chunk_end;
        let win_start = Ord::max(end - win, beg);
        let win_end = chunk_end;
        let mut bad = true;
        for (j, item) in subs.iter().enumerate().take(win_end).skip(win_start) {
            let eos = item.text.last().and_then(|c| c.trim_end().chars().last());
            if let Some(eos) = eos
                && is_end_of_sentence(&eos)
            {
                end = j + 1;
                bad = false;
            }
        }
        back = chunk_end - end;
        ret.push(&subs[beg..end]);
        if bad {
            println!(
                "Cannot split at end-of-sentence: {}-{} maps to {}-{}",
                chunk_beg, chunk_end, beg, end
            );
        }
    }
    ret.into_iter()
}

/// Entry point for the subtitle translator binary.
#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize logger - set RUST_LOG=debug to see debug output
    // Note: We rely on the user setting RUST_LOG externally, or we could set it here
    // Since set_var is unsafe in edition 2024, we just initialize with a default level
    pretty_env_logger::try_init().ok();

    // start assistant and translate subs
    let mut translator = Translator::new(args.lang).await?;
    let srt = get_parser(args.in_srt)?;
    let mut out_file = File::create(args.out_srt)?;

    for chunk in chunker(&srt.subtitles, args.chunk) {
        let translated_chunk = translator.translate_chunk(chunk, 0).await?;
        for block in translated_chunk {
            writeln!(out_file, "{}", block)?;
        }
    }

    Ok(())
}
