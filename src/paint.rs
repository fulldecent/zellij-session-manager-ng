//! The session list, the new-session layout list, rename, and the kill-all question.

use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};
use zellij_tile::prelude::{Table, Text};

use crate::chrome::{Chrome, Ink};
use crate::controls::{paint_row, Click, Piece};
use crate::model::{age_label, Found, Kind, Session};

pub struct Hit {
    pub y: usize,
    pub x0: usize,
    pub x1: usize,
    pub click: Click,
}

pub struct Frame {
    pub lines: Vec<String>,
    pub hits: Vec<Hit>,
    pub cursor: Option<(usize, usize)>,
    pub table: Option<Table>,
    pub table_at: (usize, usize, usize, usize),
    pub overlay: Vec<(Text, usize, usize)>,
}

pub struct Pointer {
    pub y: Option<usize>,
    pub x: Option<usize>,
}

pub struct ListView<'a> {
    pub sessions: &'a [Session],
    pub found: &'a [Found],
    pub selected: Option<usize>,
    pub finding: bool,
    pub query: &'a str,
    pub cursor: usize,
    pub notice: Option<&'a str>,
}

pub fn paint_list(
    view: &ListView<'_>,
    chrome: &Chrome,
    rows: usize,
    cols: usize,
    pointer: Pointer,
) -> Frame {
    let mut lines = vec![String::new(); rows];
    let mut hits = Vec::new();
    let mut cursor = None;
    if rows == 0 || cols == 0 {
        return empty_frame(lines, hits, cursor);
    }
    let mut overlay = Vec::new();
    let bars = list_bars(view.finding);
    let footer = bars.len().min(rows);
    let mut y = 0usize;
    if let Some(notice) = view.notice {
        if y < rows.saturating_sub(footer) {
            overlay.push((Text::new(notice).color_range(3, ..), 0, y));
            put(&mut lines, y, clear_line());
            y += 1;
        }
    }
    if y < rows.saturating_sub(footer) {
        let prompt = "Find: ";
        let typed: String = view.query.chars().take(view.cursor).collect();
        let cursor_x = UnicodeWidthStr::width(prompt) + UnicodeWidthStr::width(typed.as_str());
        let find = Text::new(format!("{prompt}{}_", view.query))
            .color_range(2, 0..prompt.len())
            .color_range(1, prompt.len()..);
        overlay.push((find, 0, y));
        put(&mut lines, y, clear_line());
        if view.finding {
            cursor = Some((cursor_x.min(cols.saturating_sub(1)), y));
        }
        y += 1;
    }
    let body_top = y + 1;
    let body_height = rows.saturating_sub(footer).saturating_sub(body_top);
    let start = window(view.found.len(), view.selected.unwrap_or(0), body_height);
    let mut table = Table::new();
    let mut painted = 0usize;
    for slot in 0..body_height {
        let row_y = body_top + slot;
        put(&mut lines, row_y, clear_line());
        let Some(found) = view.found.get(start + slot) else {
            continue;
        };
        let session = &view.sessions[found.index];
        let selected = view.selected == Some(start + slot);
        table = table.add_styled_row(session_row(session, &found.hits, selected));
        hits.push(Hit {
            y: row_y,
            x0: 0,
            x1: cols,
            click: Click::Row(start + slot),
        });
        painted += 1;
    }
    place_bars(&mut lines, &mut hits, &bars, rows, cols, chrome, &pointer);
    Frame {
        lines,
        hits,
        cursor,
        table: if painted == 0 { None } else { Some(table) },
        table_at: (0, body_top, cols, body_height),
        overlay,
    }
}

#[allow(clippy::too_many_arguments)]
pub fn paint_page(
    notice: Option<&str>,
    body: &[String],
    name_cursor_x: Option<usize>,
    bars: &[Vec<Piece>],
    chrome: &Chrome,
    rows: usize,
    cols: usize,
    pointer: Pointer,
) -> Frame {
    let mut lines = vec![String::new(); rows];
    let mut hits = Vec::new();
    let mut cursor = None;
    if rows == 0 || cols == 0 {
        return empty_frame(lines, hits, cursor);
    }
    let footer = bars.len().min(rows);
    let mut y = 0usize;
    if let Some(notice) = notice {
        if y < rows.saturating_sub(footer) {
            put(&mut lines, y, body_line(notice, cols, chrome));
            y += 1;
        }
    }
    let body_height = rows.saturating_sub(footer).saturating_sub(y);
    let mut layout_index = 0usize;
    for slot in 0..body_height {
        let text = body.get(slot).map(String::as_str).unwrap_or("");
        let (shown, layout) = if let Some(rest) = text.strip_prefix('\u{1}') {
            (rest, Some(true))
        } else if let Some(rest) = text.strip_prefix('\u{2}') {
            (rest, Some(false))
        } else {
            (text, None)
        };
        put(
            &mut lines,
            y + slot,
            if layout == Some(true) {
                colored(shown, cols, chrome.selected_fg, chrome.selected_bg)
            } else {
                body_line(shown, cols, chrome)
            },
        );
        if layout.is_some() {
            hits.push(Hit {
                y: y + slot,
                x0: 0,
                x1: cols,
                click: Click::Layout(layout_index),
            });
            layout_index += 1;
        }
    }
    if let Some(x) = name_cursor_x {
        cursor = Some((x.min(cols.saturating_sub(1)), y));
    }
    place_bars(&mut lines, &mut hits, bars, rows, cols, chrome, &pointer);
    Frame {
        lines,
        hits,
        cursor,
        table: None,
        table_at: (0, 0, 0, 0),
        overlay: Vec::new(),
    }
}

fn list_bars(finding: bool) -> Vec<Vec<Piece>> {
    if finding {
        vec![vec![
            key("↑", "Up", Click::Up),
            key("↓", "Down", Click::Down),
            key("TAB", "Complete", Click::Complete),
            key("ENTER", "Attach", Click::Attach),
            key("ESC", "Clear", Click::Clear),
        ]]
    } else {
        vec![
            vec![
                key("↑", "Up", Click::Up),
                key("↓", "Down", Click::Down),
                key("/", "Find", Click::Find),
                key("ENTER", "Attach", Click::Attach),
                key("N", "New", Click::New),
                key("ESC", "Close", Click::Close),
            ],
            vec![
                key("R", "Rename", Click::Rename),
                key("DEL", "Kill", Click::Kill),
                key("X", "Disconnect", Click::Disconnect),
                key_short("^A", "Save", "Save", Click::Save),
                key_short("^D", "Kill All", "Kill", Click::KillAll),
            ],
        ]
    }
}

pub fn new_bars() -> Vec<Piece> {
    vec![
        key("↑", "Up", Click::Up),
        key("↓", "Down", Click::Down),
        key("ENTER", "Create", Click::Confirm),
        key("ESC", "Back", Click::Cancel),
    ]
}

pub fn rename_bars() -> Vec<Piece> {
    vec![
        key("ENTER", "Save", Click::Confirm),
        key("ESC", "Cancel", Click::Cancel),
    ]
}

pub fn kill_bars() -> Vec<Piece> {
    vec![
        key("ENTER", "Kill", Click::Confirm),
        key("ESC", "Cancel", Click::Cancel),
    ]
}

fn key(key: &'static str, label: &'static str, click: Click) -> Piece {
    key_short(key, label, label, click)
}

fn key_short(key: &'static str, label: &'static str, short: &'static str, click: Click) -> Piece {
    Piece::Key {
        key,
        label,
        short,
        click,
    }
}

fn place_bars(
    lines: &mut [String],
    hits: &mut Vec<Hit>,
    bars: &[Vec<Piece>],
    rows: usize,
    cols: usize,
    chrome: &Chrome,
    pointer: &Pointer,
) {
    let start = rows.saturating_sub(bars.len());
    for (offset, pieces) in bars.iter().enumerate() {
        let y = start + offset;
        if y >= rows {
            break;
        }
        let hover_x = if pointer.y == Some(y) {
            pointer.x
        } else {
            None
        };
        let (ansi, row_hits) = paint_row(pieces, cols, chrome, hover_x);
        lines[y] = ansi;
        for (x0, x1, click) in row_hits {
            hits.push(Hit { y, x0, x1, click });
        }
    }
}

fn session_row(session: &Session, hits: &[usize], selected: bool) -> Vec<Text> {
    let mut name = Text::new(session.name.clone()).color_range(1, ..);
    if !hits.is_empty() {
        name = name.color_indices(3, hits.to_vec());
    }
    let (details_text, ranges) = session_details(session);
    let mut details = Text::new(details_text);
    for range in ranges {
        details = details.color_range(2, range);
    }
    let tag = Text::new(session_tag(session)).color_range(0, ..);
    if selected {
        vec![name.selected(), details.selected(), tag.selected()]
    } else {
        vec![name, details, tag]
    }
}

fn session_details(session: &Session) -> (String, Vec<std::ops::Range<usize>>) {
    match session.kind {
        Kind::Running => {
            let panes = session.panes.to_string();
            let clients = session.clients.to_string();
            let word = if session.clients == 1 {
                "client"
            } else {
                "clients"
            };
            let text = format!("{panes} panes, {clients} {word}");
            let pane_end = panes.len();
            let client_start = pane_end + " panes, ".len();
            (
                text,
                vec![0..pane_end, client_start..client_start + clients.len()],
            )
        }
        Kind::Exited => {
            let age = age_label(session.age);
            let text = format!("Created {age}");
            let start = "Created ".len();
            let ranges = std::iter::once(start..start + age.len()).collect();
            (text, ranges)
        }
    }
}

fn session_tag(session: &Session) -> &'static str {
    match session.kind {
        Kind::Running if session.current => "[CURRENT]",
        Kind::Running => "[ATTACH]",
        Kind::Exited => "[RESURRECT]",
    }
}

fn clear_line() -> String {
    "\u{1b}[0m\u{1b}[K".to_string()
}

fn empty_frame(lines: Vec<String>, hits: Vec<Hit>, cursor: Option<(usize, usize)>) -> Frame {
    Frame {
        lines,
        hits,
        cursor,
        table: None,
        table_at: (0, 0, 0, 0),
        overlay: Vec::new(),
    }
}

fn body_line(text: &str, cols: usize, chrome: &Chrome) -> String {
    colored(text, cols, chrome.light, chrome.bar)
}

fn colored(text: &str, cols: usize, fg: Ink, bg: Ink) -> String {
    let shown = truncate(text, cols);
    let gap = cols.saturating_sub(UnicodeWidthStr::width(shown.as_str()));
    format!(
        "\u{1b}[0m\u{1b}[{};{}m{shown}{}\u{1b}[0m",
        fg.fg(),
        bg.bg(),
        " ".repeat(gap)
    )
}

fn truncate(text: &str, cols: usize) -> String {
    if UnicodeWidthStr::width(text) <= cols {
        return text.to_string();
    }
    let budget = cols.saturating_sub(3);
    let mut out = String::new();
    let mut used = 0usize;
    for ch in text.chars() {
        let width = UnicodeWidthChar::width(ch).unwrap_or(0);
        if used + width > budget {
            break;
        }
        out.push(ch);
        used += width;
    }
    out.push_str("...");
    out
}

fn window(len: usize, selected: usize, height: usize) -> usize {
    if height == 0 || len <= height {
        return 0;
    }
    let mut start = selected.saturating_sub(height / 2);
    if start + height > len {
        start = len - height;
    }
    start
}

fn put(lines: &mut [String], y: usize, text: String) {
    if y < lines.len() {
        lines[y] = text;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chrome::Chrome;
    use crate::controls::visible;

    #[test]
    fn the_list_shows_find_and_attach() {
        let frame = paint_list(
            &ListView {
                sessions: &[],
                found: &[],
                selected: None,
                finding: true,
                query: "",
                cursor: 0,
                notice: None,
            },
            &Chrome::default(),
            18,
            72,
            Pointer { y: None, x: None },
        );
        let text = frame
            .lines
            .iter()
            .map(|line| visible(line))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(!frame.overlay.is_empty());
        assert!(text.contains("ENTER Attach"));
        assert!(text.contains("ESC Clear"));
        assert!(!text.contains("Sessions"));
        assert!(frame.cursor.is_some());
    }
}
