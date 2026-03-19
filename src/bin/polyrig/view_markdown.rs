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

use termimad::{Area, MadSkin, MadView};

/// Build a markdown string from the conversation history.
pub fn build_history_markdown(history: &[String]) -> String {
    let mut md = String::new();
    for (i, msg) in history.iter().enumerate() {
        let role = if i % 2 == 0 { "User" } else { "Assistant" };
        md.push_str(&format!("**{}:**\n\n{}\n\n---\n\n", role, msg.trim()));
    }
    md
}

fn draw_info_bar(cols: u16, rows: u16, info_bar: &str) {
    use std::io::Write;
    use termimad::crossterm::ExecutableCommand;
    use termimad::crossterm::cursor::MoveTo;
    use termimad::crossterm::terminal::{Clear, ClearType};
    let bar_width = info_bar.len();
    if bar_width <= cols as usize {
        let row = rows - 1;
        let _ = std::io::stdout().execute(MoveTo(0, row));
        let _ = std::io::stdout().execute(Clear(ClearType::FromCursorDown));
        let start = ((cols as usize).saturating_sub(bar_width)) / 2;
        let _ = std::io::stdout().execute(MoveTo(start as u16, row));
        print!("{}", info_bar);
        let _ = std::io::stdout().flush();
    }
}

/// Display an interactive scrollable view of markdown content.
///
/// Handles PgUp/PgDn, up/down arrows, mouse wheel, and q to quit.
/// Uses crossterm for terminal input and alternate screen mode.
pub fn show_scrolled_view(history: &[String]) {
    use termimad::crossterm::ExecutableCommand;
    use termimad::crossterm::event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, MouseEventKind,
    };
    use termimad::crossterm::terminal::{
        Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
        enable_raw_mode,
    };

    // Hide cursor.
    print!("\x1B[?25l");

    // Enter alternate screen for a clean display.
    if let Err(e) = std::io::stdout().execute(EnterAlternateScreen) {
        eprintln!("Terminal error: {}", e);
        return;
    }
    let _ = std::io::stdout().execute(Clear(ClearType::All));

    // Enable raw mode and mouse capture for scrolling.
    if let Err(e) = enable_raw_mode() {
        eprintln!("Terminal error: {}", e);
        let _ = std::io::stdout().execute(LeaveAlternateScreen);
        return;
    }
    if let Err(e) = std::io::stdout().execute(EnableMouseCapture) {
        eprintln!("Mouse capture error: {}", e);
    }

    // Detect terminal size. In alternate screen mode, terminal_size() returns
    // the actual dimensions.
    let (term_w, term_h) = termimad::terminal_size();
    let cols = term_w.max(40);
    let rows = term_h.max(10);

    let text = build_history_markdown(history);

    let skin = MadSkin::default();
    let area = Area::new(0, 0, cols, rows.saturating_sub(1).max(1));

    // Start the viewport at the top of the latest message.
    let last_msg_scroll = if history.len() > 1 {
        let prefix_md = build_history_markdown(&history[..history.len().saturating_sub(1)]);
        let prefix_text = skin.area_text(&prefix_md, &area);
        prefix_text.lines.len()
    } else {
        0
    };

    let mut view = MadView::from(text, area, skin);
    view.scroll = last_msg_scroll;

    let info_bar = " ↑↑↓↓/j/k: scroll  PgUp/PgDn: page  q/Q/Esc: quit ";

    // Initial render.
    if let Err(e) = view.write() {
        eprintln!("Render error: {}", e);
    }
    draw_info_bar(cols, rows, info_bar);

    // Interactive event loop for scrolling.
    loop {
        if let Err(_e) = event::poll(std::time::Duration::from_millis(100)) {
            break;
        }
        match event::read() {
            Ok(Event::Key(key)) => {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                match key.code {
                    KeyCode::PageUp => {
                        view.try_scroll_pages(-1);
                    }
                    KeyCode::PageDown => {
                        view.try_scroll_pages(1);
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        view.try_scroll_lines(-1);
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        view.try_scroll_lines(1);
                    }
                    KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('Q') => {
                        break;
                    }
                    _ => continue,
                }
            }
            Ok(Event::Mouse(mouse)) => match mouse.kind {
                MouseEventKind::ScrollUp => {
                    view.try_scroll_lines(-1);
                }
                MouseEventKind::ScrollDown => {
                    view.try_scroll_lines(1);
                }
                _ => {}
            },
            _ => {}
        }

        // Re-render after scroll or key event.
        if let Err(e) = view.write() {
            eprintln!("Render error: {}", e);
            break;
        }

        draw_info_bar(cols, rows, info_bar);
    }

    // Cleanup: show cursor, disable mouse, leave alternate screen, restore
    // normal terminal mode.
    let _ = std::io::stdout().execute(DisableMouseCapture);
    let _ = std::io::stdout().execute(LeaveAlternateScreen);
    let _ = disable_raw_mode();
    print!("\x1B[?25h");
}
