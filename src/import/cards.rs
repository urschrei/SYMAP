//! Writing the deck's cards and comments.

use super::topology::P;
use crate::Error;
use std::fmt::Write;

/// A number right-justified in a 10-column field, with at most `decimals`
/// decimals (fewer if it would not fit), trailing zeros dropped.
pub(crate) fn field_with(x: f64, decimals: usize) -> Result<String, Error> {
    (0..=decimals)
        .rev()
        .map(|d| {
            let s = format!("{x:.d$}");
            let s = if s.contains('.') {
                let t = s.trim_end_matches('0');
                if t.ends_with('.') {
                    format!("{t}0")
                } else {
                    t.to_string()
                }
            } else {
                format!("{s}.")
            };
            format!("{s:>10}")
        })
        .find(|s| s.len() <= 10)
        .ok_or_else(|| Error::general(format!("{x} does not fit in a 10-column field")))
}

/// A value, with up to six decimals.
pub(crate) fn field(x: f64) -> Result<String, Error> {
    field_with(x, 6)
}

/// A location to the nearest thousandth (a metre, in kilometres), after
/// `head` in columns 1-10.
pub(crate) fn coordinate_card(head: &str, (down, across): P) -> Result<String, Error> {
    Ok(format!(
        "{head:10}{}{}",
        field_with(down, 3)?,
        field_with(across, 3)?
    ))
}

/// Comment lines, wrapped to fit the card.
pub(crate) fn comment(out: &mut String, text: &str) {
    let mut line = String::from("#");
    for word in text.split_whitespace() {
        if line.len() + 1 + word.len() > 78 && line.len() > 1 {
            let _ = writeln!(out, "{line}");
            line = String::from("# ");
            line.push_str(word);
        } else {
            line.push(' ');
            line.push_str(word);
        }
    }
    let _ = writeln!(out, "{line}");
}

pub(crate) fn card_text(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_control())
        .take(80)
        .collect::<String>()
        .to_uppercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fields_fit_ten_columns() {
        assert_eq!(field(3.5).unwrap(), "       3.5");
        assert_eq!(field(48.0).unwrap(), "      48.0");
        assert_eq!(field(-12345.6789).unwrap(), "-12345.679");
        assert_eq!(field(0.000012).unwrap(), "  0.000012");
        assert_eq!(field(123456789.0).unwrap(), "123456789.");
        assert_eq!(field_with(2.0 / 3.0, 3).unwrap(), "     0.667");
        assert!(field(1e12).is_err());
    }
}
