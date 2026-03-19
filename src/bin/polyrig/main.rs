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
use clap::Parser;
use polyrig::talks::Talk;
use polyrig::talks::stream_messages;
use termimad::MadSkin;
use tokio_stream::StreamExt;

mod view_markdown;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Command-line arguments for selecting a conversation
    #[command(subcommand)]
    talk: Talk,
}

/// Reads a multi-line user message from standard input.
fn read_msg(rl: &mut rustyline::DefaultEditor) -> Option<String> {
    let mut msg = String::new();
    while let Ok(line) = if msg.is_empty() {
        rl.readline("> ")
    } else {
        rl.readline("… ")
    } {
        if line.is_empty() {
            break;
        }
        // add line to message
        msg.push_str(&line);
        msg.push('\n');
    }
    if msg.is_empty() { None } else { Some(msg) }
}

/// Entry point for the CLI binary.
#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let talk = args.talk;

    if !talk.runs_on_cli() {
        return Err(anyhow::anyhow!(
            "The talk '{}' cannot be run via the CLI.",
            talk
        ));
    }

    let mut conversation = talk.get_conv().await?;

    if let Some(msg) = &conversation.first_msg {
        println!("{}\n", msg);
    }

    let mut rl = rustyline::DefaultEditor::new()?;
    let mut history: Vec<String> = Vec::new();

    while let Some(user_msg) = read_msg(&mut rl) {
        // Use the stream_response method to handle the conversation
        let stream = conversation.stream_response(user_msg.clone()).await;
        let mut msg_stream = std::pin::pin!(stream_messages(stream));
        let mut full_response = String::new();

        // During streaming, print raw text (no markdown formatting).
        // This avoids screen-clearing flicker and lets the user read
        // the response as it arrives.
        while let Some(chunk) = msg_stream.next().await {
            match chunk {
                Ok(txt) => {
                    print!("{}", txt);
                    full_response.push_str(&txt);
                }
                Err(e) => eprintln!("Error: {}", e),
            }
        }

        println!(); // Trailing newline after streaming.

        // Add both messages to history for the complete scrollable view.
        history.push(user_msg);
        history.push(full_response.clone());

        // Keep the conversation's internal history in sync so the agent's
        // context window grows correctly and trim_history() is applied.
        conversation.add_assistant_response(full_response.clone());

        // Show formatted, scrollable view of the complete conversation.
        view_markdown::show_scrolled_view(&history);

        // Re-render the latest response with markdown formatting after exiting
        // the scroll view.
        println!();
        let skin = MadSkin::default();
        println!("{}", skin.term_text(&full_response));
    }

    Ok(())
}
