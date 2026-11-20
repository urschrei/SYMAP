//! Printouts for a colour terminal. A terminal cell holds one character, so
//! an overprinted stack prints as its stand-in (`Strikes::plain`) on grey
//! paper that makes up the rest of the stack's ink: the cell's overall tone
//! matches the overprint, as on the line printer.

use crate::page::Line;
use crate::symbols::{Strikes, ink};

/// The colours a terminal can show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Palette {
    /// 24-bit colour (`COLORTERM=truecolor`).
    TrueColor,
    /// The xterm 256-colour palette's greys.
    Ansi256,
}

const PAPER: [u8; 3] = [251, 251, 244];
const INK: [u8; 3] = [30, 28, 40];

/// The paper's colour where `d` (0 to 1) of it is inked.
fn shade(d: f64) -> [u8; 3] {
    let d = d.clamp(0.0, 1.0);
    std::array::from_fn(|i| {
        (f64::from(PAPER[i]) + (f64::from(INK[i]) - f64::from(PAPER[i])) * d).round() as u8
    })
}

/// The nearest xterm-256 grey: white (231), the 24-step ramp (232-255,
/// `8 + 10k`), or black (16).
fn grey256(rgb: [u8; 3]) -> u8 {
    let y = (0.2126 * f64::from(rgb[0]) + 0.7152 * f64::from(rgb[1]) + 0.0722 * f64::from(rgb[2]))
        .round();
    let (mut best, mut err) = (231, (255.0 - y).abs());
    for (code, level) in (232..=255)
        .map(|c| (c, f64::from(8 + 10 * (c - 232))))
        .chain([(16, 0.0)])
    {
        if (level - y).abs() < err {
            (best, err) = (code, (level - y).abs());
        }
    }
    best
}

fn sgr(out: &mut String, palette: Palette, layer: u8, rgb: [u8; 3]) {
    use std::fmt::Write;
    match palette {
        Palette::TrueColor => write!(out, "\x1b[{layer}8;2;{};{};{}m", rgb[0], rgb[1], rgb[2]),
        Palette::Ansi256 => write!(out, "\x1b[{layer}8;5;{}m", grey256(rgb)),
    }
    .unwrap();
}

/// The character to print for a stack, and how dark its paper is.
fn cell(s: &Strikes) -> (char, f64) {
    let c = s.coverage();
    let glyph = match s.plain() {
        // A solid block outweighs the printed stack: its darkest strike on
        // grey paper keeps the step from the level below it in proportion.
        '█' => s.darkest(),
        g => g,
    };
    let g = match glyph {
        'Θ' | '⊗' => ink('O'),
        _ => ink(glyph),
    };
    // The paper the glyph leaves bare takes the rest of the stack's ink.
    (glyph, (1.0 - (1.0 - c) / (1.0 - g)).max(0.0))
}

/// A page's lines as ANSI-coloured text: dark ink on a sheet of paper as
/// wide as its widest line, whatever the terminal's own colours.
pub fn ansi(page: &[Line], palette: Palette) -> String {
    let passes: Vec<Vec<Vec<char>>> = page
        .iter()
        .map(|l| l.strikes.iter().map(|s| s.chars().collect()).collect())
        .collect();
    let width = passes
        .iter()
        .flat_map(|l| l.iter().map(Vec::len))
        .max()
        .unwrap_or(0);
    let mut out = String::new();
    for (l, line) in page.iter().zip(&passes) {
        if l.eject {
            out.push('\x0c');
        }
        sgr(&mut out, palette, 3, INK);
        let mut paper = None;
        for col in 0..width {
            let mut s = Strikes::BLANK;
            for (k, pass) in line.iter().take(4).enumerate() {
                s.0[k] = pass.get(col).copied().unwrap_or(' ');
            }
            let (glyph, d) = cell(&s);
            let bg = shade(d);
            if paper != Some(bg) {
                sgr(&mut out, palette, 4, bg);
                paper = Some(bg);
            }
            out.push(glyph);
        }
        out.push_str("\x1b[0m\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overprints_shade_the_paper() {
        // A single strike prints as itself on clean paper.
        assert_eq!(cell(&Strikes::new("X")), ('X', 0.0));
        // Paper darkens through the overprinted levels.
        let (g, d) = cell(&Strikes::new("O-"));
        assert_eq!(g, 'Θ');
        let (ox, oxav) = (cell(&Strikes::new("OX")), cell(&Strikes::new("OXAV")));
        assert_eq!(oxav.0, 'O');
        assert!(0.0 < d && d < ox.1 && ox.1 < oxav.1 && oxav.1 < 1.0);
    }

    #[test]
    fn greys() {
        assert_eq!(grey256([255, 255, 255]), 231);
        assert_eq!(grey256([0, 0, 0]), 16);
        assert_eq!(grey256([128, 128, 128]), 244);
    }

    #[test]
    fn sheet() {
        let page = [
            Line::from_strikes(&[Strikes::new("X"), Strikes::new("OX")]),
            Line::text("A"),
        ];
        let s = ansi(&page, Palette::Ansi256);
        let lines: Vec<&str> = s.lines().collect();
        assert_eq!(lines.len(), 2);
        // Each line resets, and is padded to the sheet's width.
        assert!(lines.iter().all(|l| l.ends_with("\x1b[0m")));
        assert!(lines[0].contains("X") && lines[0].contains('⊗'));
        assert!(lines[1].contains("A "));
        // The overprinted cell changes the paper.
        assert_eq!(lines[0].matches("\x1b[48;5;").count(), 2);
    }
}
