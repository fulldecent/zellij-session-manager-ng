//! One row of keys, labels, and choice ribbons.
//!
//! A labeled button is a reversed key, a label, and one pad on each side.
//! Those cells share one click. A choice uses the tab ribbon: selected,
//! unselected, and hover. A title is not a click.

use unicode_width::UnicodeWidthStr;

use crate::chrome::{Chrome, Ink};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Click {
    Up,
    Down,
    Complete,
    Attach,
    Find,
    Clear,
    Close,
    New,
    Rename,
    Kill,
    Disconnect,
    Save,
    KillAll,
    Confirm,
    Cancel,
    Layout(usize),
    Row(usize),
}

pub enum Piece {
    /// A help title. It is not a key, and it is not a click.
    #[allow(dead_code)]
    Title(&'static str),
    Key {
        key: &'static str,
        label: &'static str,
        /// Used when the full label does not fit. Empty means the full label is already short.
        short: &'static str,
        click: Click,
    },
    #[allow(dead_code)]
    Choice {
        label: &'static str,
        on: bool,
        click: Click,
    },
}

struct Drawn {
    text: String,
    look: Look,
    click: Option<Click>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Look {
    Title,
    Key,
    Label,
    ChoiceOn,
    ChoiceOff,
    Ellipsis,
}

pub fn paint_row(
    pieces: &[Piece],
    cols: usize,
    chrome: &Chrome,
    hover_x: Option<usize>,
) -> (String, Vec<(usize, usize, Click)>) {
    if cols == 0 {
        return (String::new(), Vec::new());
    }
    let mut drawn = fit(pieces, cols);
    let gap = cols.saturating_sub(width(&drawn));
    if gap > 0 {
        drawn.push(Drawn {
            text: " ".repeat(gap),
            look: Look::Label,
            click: None,
        });
    }
    let hits = hits_of(&drawn);
    (paint(&drawn, chrome, hover_x), hits)
}

fn fit(pieces: &[Piece], cols: usize) -> Vec<Drawn> {
    let mut shortened = false;
    let mut labels = button_labels(pieces, false);
    loop {
        let drawn = draw(pieces, &labels);
        if width(&drawn) <= cols || drawn.is_empty() {
            return if width(&drawn) <= cols {
                drawn
            } else {
                vec![Drawn {
                    text: ellipsis(cols),
                    look: Look::Ellipsis,
                    click: None,
                }]
            };
        }
        if !shortened && has_shorter_label(pieces) {
            shortened = true;
            labels = button_labels(pieces, true);
            continue;
        }
        if let Some(slot) = labels.iter_mut().rposition(|label| label.is_some()) {
            labels[slot] = None;
            continue;
        }
        if pieces.len() > 1 {
            let shorter = &pieces[..pieces.len() - 1];
            let mut next = fit(shorter, cols.saturating_sub(4));
            if let Some(last) = next.iter_mut().rev().find(|piece| piece.click.is_some()) {
                last.click = None;
            }
            let mark = " ...";
            if width(&next) + UnicodeWidthStr::width(mark) <= cols {
                next.push(Drawn {
                    text: mark.to_string(),
                    look: Look::Ellipsis,
                    click: None,
                });
            }
            return next;
        }
        return vec![Drawn {
            text: ellipsis(cols),
            look: Look::Ellipsis,
            click: None,
        }];
    }
}

fn button_labels(pieces: &[Piece], shortened: bool) -> Vec<Option<&str>> {
    pieces
        .iter()
        .map(|piece| match piece {
            Piece::Key { label, short, .. } if !label.is_empty() => {
                if shortened && !short.is_empty() {
                    Some(*short)
                } else {
                    Some(*label)
                }
            }
            _ => None,
        })
        .collect()
}

fn has_shorter_label(pieces: &[Piece]) -> bool {
    pieces.iter().any(|piece| match piece {
        Piece::Key { label, short, .. } => !short.is_empty() && *short != *label,
        _ => false,
    })
}

fn draw(pieces: &[Piece], labels: &[Option<&str>]) -> Vec<Drawn> {
    let mut drawn = Vec::new();
    for (index, piece) in pieces.iter().enumerate() {
        match piece {
            Piece::Title(text) => {
                drawn.push(Drawn {
                    text: format!("{text} "),
                    look: Look::Title,
                    click: None,
                });
            }
            Piece::Key { key, click, .. } => {
                let label = labels.get(index).copied().flatten().unwrap_or("");
                if label.is_empty() {
                    if index > 0 {
                        drawn.push(Drawn {
                            text: " ".to_string(),
                            look: Look::Label,
                            click: None,
                        });
                    }
                    drawn.push(Drawn {
                        text: (*key).to_string(),
                        look: Look::Key,
                        click: Some(*click),
                    });
                } else {
                    drawn.push(Drawn {
                        text: " ".to_string(),
                        look: Look::Label,
                        click: Some(*click),
                    });
                    drawn.push(Drawn {
                        text: (*key).to_string(),
                        look: Look::Key,
                        click: Some(*click),
                    });
                    drawn.push(Drawn {
                        text: format!(" {label} "),
                        look: Look::Label,
                        click: Some(*click),
                    });
                }
            }
            Piece::Choice { label, on, click } => {
                drawn.push(Drawn {
                    text: format!(" {label} "),
                    look: if *on { Look::ChoiceOn } else { Look::ChoiceOff },
                    click: Some(*click),
                });
            }
        }
    }
    drawn
}

fn ellipsis(cols: usize) -> String {
    let mark = "...";
    if UnicodeWidthStr::width(mark) <= cols {
        mark.to_string()
    } else {
        String::new()
    }
}

fn width(drawn: &[Drawn]) -> usize {
    drawn
        .iter()
        .map(|piece| UnicodeWidthStr::width(piece.text.as_str()))
        .sum()
}

fn hits_of(drawn: &[Drawn]) -> Vec<(usize, usize, Click)> {
    let mut hits = Vec::new();
    let mut x = 0usize;
    for piece in drawn {
        let piece_width = UnicodeWidthStr::width(piece.text.as_str());
        if let Some(click) = piece.click {
            if let Some((_, end, last)) = hits.last_mut() {
                if *last == click && *end == x {
                    *end = x + piece_width;
                    x += piece_width;
                    continue;
                }
            }
            hits.push((x, x + piece_width, click));
        }
        x += piece_width;
    }
    hits
}

fn paint(drawn: &[Drawn], chrome: &Chrome, hover_x: Option<usize>) -> String {
    let hover = hover_x.and_then(|column| click_at(drawn, column));
    let grouped = hover.is_some_and(|click| labeled(drawn, click));
    let mut out = String::new();
    for piece in drawn {
        let hot = piece.click.is_some() && piece.click == hover;
        let (fg, bg) = colors(piece.look, hot, grouped, chrome);
        out.push_str("\u{1b}[0m");
        out.push_str(&format!("\u{1b}[{};{}m", fg.fg(), bg.bg()));
        out.push_str(&piece.text);
    }
    out.push_str("\u{1b}[0m");
    out
}

fn click_at(drawn: &[Drawn], column: usize) -> Option<Click> {
    let mut x = 0usize;
    for piece in drawn {
        let piece_width = UnicodeWidthStr::width(piece.text.as_str());
        if column >= x && column < x + piece_width {
            return piece.click;
        }
        x += piece_width;
    }
    None
}

fn labeled(drawn: &[Drawn], click: Click) -> bool {
    drawn.iter().any(|piece| {
        piece.click == Some(click)
            && piece.look == Look::Label
            && piece.text.chars().any(|ch| ch != ' ')
    })
}

fn colors(look: Look, hot: bool, grouped: bool, chrome: &Chrome) -> (Ink, Ink) {
    if hot && grouped {
        return (chrome.dark, chrome.light);
    }
    match look {
        Look::Title | Look::Ellipsis => (chrome.alt, chrome.bar),
        Look::Key => {
            if hot {
                (chrome.light, chrome.dark)
            } else {
                (chrome.dark, chrome.light)
            }
        }
        Look::Label => (chrome.light, chrome.bar),
        Look::ChoiceOn => (chrome.selected_fg, chrome.selected_bg),
        Look::ChoiceOff => {
            if hot {
                (chrome.dark, chrome.hover)
            } else {
                (chrome.dark, chrome.light)
            }
        }
    }
}

#[cfg(test)]
pub fn visible(rendered: &str) -> String {
    let mut out = String::new();
    let mut chars = rendered.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' {
            if chars.peek() == Some(&'[') {
                chars.next();
                for next in chars.by_ref() {
                    if next.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
            continue;
        }
        out.push(ch);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_wide_row_names_attach() {
        let pieces = [
            Piece::Key {
                key: "ENTER",
                label: "Attach",
                short: "Attach",
                click: Click::Attach,
            },
            Piece::Key {
                key: "ESC",
                label: "Close",
                short: "Close",
                click: Click::Close,
            },
        ];
        let (ansi, hits) = paint_row(&pieces, 40, &Chrome::default(), None);
        let text = visible(&ansi);
        assert!(text.contains("ENTER Attach"));
        assert!(text.contains("ESC Close"));
        assert_eq!(hits.len(), 2);
        assert_eq!(hit_at(&hits, 1), Some(Click::Attach));
    }

    fn hit_at(hits: &[(usize, usize, Click)], column: usize) -> Option<Click> {
        hits.iter()
            .find(|(start, end, _)| column >= *start && column < *end)
            .map(|(_, _, click)| *click)
    }
}
