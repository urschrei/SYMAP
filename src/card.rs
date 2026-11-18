//! Helpers for reading 80-column card images.

/// Characters in 1-based inclusive columns `from..=to`. Short lines read as blank.
pub fn cols(line: &str, from: usize, to: usize) -> String {
    line.chars().skip(from - 1).take(to + 1 - from).collect()
}

/// Parse a Fortran-style real field: blank is `Some(None)`, junk is `None`.
pub fn real_field(s: &str) -> Option<Option<f64>> {
    let t = s.trim();
    if t.is_empty() {
        return Some(None);
    }
    finite(t).map(Some)
}

fn finite(t: &str) -> Option<f64> {
    t.parse::<f64>().ok().filter(|v| v.is_finite())
}

/// Read the first `n` numbers from a data card.
///
/// Whitespace-separated values are tried first; failing that, the card is
/// read as consecutive 10-column fields (`nF10.x`), which handles punched
/// decks where adjacent fields run together.
pub fn numbers(line: &str, n: usize) -> Option<Vec<f64>> {
    let toks: Vec<&str> = line.split_whitespace().collect();
    if toks.len() >= n {
        if let Some(v) = toks[..n]
            .iter()
            .map(|t| finite(t))
            .collect::<Option<Vec<_>>>()
        {
            return Some(v);
        }
    }
    (0..n)
        .map(|i| finite(cols(line, 10 * i + 1, 10 * i + 10).trim()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn columns_are_one_based_and_padded() {
        assert_eq!(cols("abcdef", 2, 4), "bcd");
        assert_eq!(cols("ab", 4, 5), "");
    }

    #[test]
    fn free_and_fixed_numbers() {
        assert_eq!(numbers("  1.5   2", 2), Some(vec![1.5, 2.0]));
        // Two F10.2 fields with no separating blank.
        assert_eq!(
            numbers("    123.45-1234.567", 2),
            Some(vec![123.45, -1234.567])
        );
        assert_eq!(numbers("x y", 2), None);
        assert_eq!(numbers("nan 1", 2), None);
    }
}
