//! Theme colors for keys, labels, and ribbons.
//!
//! Indexes are the fallback from `Styling::from(default_palette())` until a
//! mode update arrives. Zellij Style Guide Next Generation names the fields.

use zellij_tile::prelude::{PaletteColor, Style};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Chrome {
    pub bar: Ink,
    pub dark: Ink,
    pub light: Ink,
    pub ctrl: Ink,
    pub alt: Ink,
    pub hover: Ink,
    pub selected_fg: Ink,
    pub selected_bg: Ink,
}

impl Default for Chrome {
    fn default() -> Self {
        Self {
            bar: Ink::Bit(16),
            dark: Ink::Bit(16),
            light: Ink::Bit(245),
            ctrl: Ink::Bit(255),
            alt: Ink::Bit(166),
            hover: Ink::Bit(238),
            selected_fg: Ink::Bit(16),
            selected_bg: Ink::Bit(167),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ink {
    Bit(u8),
    Rgb(u8, u8, u8),
}

impl Ink {
    pub fn fg(self) -> String {
        match self {
            Ink::Bit(n) => format!("38;5;{n}"),
            Ink::Rgb(r, g, b) => format!("38;2;{r};{g};{b}"),
        }
    }

    pub fn bg(self) -> String {
        match self {
            Ink::Bit(n) => format!("48;5;{n}"),
            Ink::Rgb(r, g, b) => format!("48;2;{r};{g};{b}"),
        }
    }
}

pub fn chrome_from(style: &Style) -> Chrome {
    let colors = &style.colors;
    Chrome {
        bar: ink(colors.text_unselected.background),
        dark: ink(colors.ribbon_unselected.base),
        light: ink(colors.ribbon_unselected.background),
        ctrl: ink(colors.text_unselected.base),
        alt: ink(colors.text_unselected.emphasis_0),
        hover: ink(colors.ribbon_unselected.emphasis_1),
        selected_fg: ink(colors.ribbon_selected.base),
        selected_bg: ink(colors.ribbon_selected.background),
    }
}

fn ink(color: PaletteColor) -> Ink {
    match color {
        PaletteColor::EightBit(n) => Ink::Bit(n),
        PaletteColor::Rgb((r, g, b)) => Ink::Rgb(r, g, b),
    }
}
