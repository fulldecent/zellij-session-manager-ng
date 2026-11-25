//! The session list, the new-session layout list, rename, and the kill-all question.

use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::chrome::{Chrome, Ink};
use crate::controls::{paint_row, Click, Piece};
use crate::model::{detail, Found, Session};

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
        return Frame {
            lines,
            hits,
            cursor,
        };
    }
    let bars = list_bars(view.finding);
    let footer = bars.len().min(rows);
    let mut y = 0usize;
    if view.notice.is_some() && y < rows.saturating_sub(footer) {
        put(
            &mut lines,
            y,
            body_line(view.notice.unwrap_or(""), cols, chrome),
        );
        y += 1;
    }
    if y < rows.saturating_sub(footer) {
        let prompt = "Find: ";
        let typed: String = view.query.chars().take(view.cursor).collect();
        let cursor_x = UnicodeWidthStr::width(prompt) + UnicodeWidthStr::width(typed.as_str());
        put(
            &mut lines,
            y,
            colored(
                &format!("{prompt}{}", view.query),
                cols,
                chrome.light,
                chrome.bar,
            ),
        );
        if view.finding {
            cursor = Some((cursor_x.min(cols.saturating_sub(1)), y));
        }
        y += 1;
    }
    let body_top = y + 1;
    let body_height = rows.saturating_sub(footer).saturating_sub(body_top);
    let start = window(view.found.len(), view.selected.unwrap_or(0), body_height);
    for slot in 0..body_height {
        let row_y = body_top + slot;
        let Some(found) = view.found.get(start + slot) else {
            put(&mut lines, row_y, body_line("", cols, chrome));
            continue;
        };
        let session = &view.sessions[found.index];
        let selected = view.selected == Some(start + slot);
        let hover = pointer.y == Some(row_y) && !selected;
        put(
            &mut lines,
            row_y,
            session_line(session, &found.hits, selected, hover, chrome, cols),
        );
        hits.push(Hit {
            y: row_y,
            x0: 0,
            x1: cols,
            click: Click::Row(start + slot),
        });
    }
    place_bars(&mut lines, &mut hits, &bars, rows, cols, chrome, &pointer);
    Frame {
        lines,
        hits,
        cursor,
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
        return Frame {
            lines,
            hits,
            cursor,
        };
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

fn session_line(
    session: &Session,
    hits: &[usize],
    selected: bool,
    hover: bool,
    chrome: &Chrome,
    cols: usize,
) -> String {
    let meta = detail(session);
    let meta_width = UnicodeWidthStr::width(meta.as_str());
    let name_cols = cols.saturating_sub(meta_width + 1);
    let name = truncate(&session.name, name_cols);
    if selected {
        let mut text = name;
        let pad = name_cols.saturating_sub(UnicodeWidthStr::width(text.as_str()));
        text.push_str(&" ".repeat(pad));
        if meta_width > 0 && cols > meta_width {
            text.push(' ');
            text.push_str(&meta);
        }
        return colored(&text, cols, chrome.selected_fg, chrome.selected_bg);
    }
    let bg = if hover { chrome.hover } else { chrome.bar };
    let mut out = String::from("\u{1b}[0m");
    let mut used = 0usize;
    for (index, ch) in name.chars().enumerate() {
        let width = UnicodeWidthChar::width(ch).unwrap_or(0);
        if used + width > name_cols {
            break;
        }
        let ink = if hits.contains(&index) {
            chrome.alt
        } else {
            chrome.light
        };
        out.push_str(&format!("\u{1b}[{};{}m{ch}", ink.fg(), bg.bg()));
        used += width;
    }
    if used < cols {
        let between = cols - used - meta_width.min(cols - used);
        let rest = format!("{}{meta}", " ".repeat(between));
        out.push_str(&format!("\u{1b}[{};{}m{rest}", chrome.light.fg(), bg.bg()));
    }
    out.push_str("\u{1b}[0m");
    out
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
        assert!(text.contains("Find:"));
        assert!(text.contains("ENTER Attach"));
        assert!(text.contains("ESC Clear"));
        assert!(!text.contains("Sessions"));
        assert!(frame.cursor.is_some());
    }
}
