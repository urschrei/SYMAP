//! A CSV reader for the importer: a header row, then one record per line.
//! Commas, semicolons or tabs separate fields (whichever the header uses
//! most); fields may be quoted, with `""` for a quote.

use crate::Error;

pub struct Table {
    pub header: Vec<String>,
    /// Each record with the line it starts on.
    pub rows: Vec<(usize, Vec<String>)>,
}

pub fn parse(text: &str) -> Result<Table, Error> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let first = text.lines().next().unwrap_or("");
    let sep = [',', ';', '\t']
        .into_iter()
        .max_by_key(|&c| first.matches(c).count())
        .unwrap_or(',');
    let mut records = Vec::new();
    let mut field = String::new();
    let mut record = Vec::new();
    let mut quoted = false;
    let mut line = 1;
    let mut start = 1;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if quoted && chars.peek() == Some(&'"') => {
                chars.next();
                field.push('"');
            }
            '"' if quoted => quoted = false,
            '"' if field.trim().is_empty() => {
                field.clear();
                quoted = true;
            }
            '\n' if quoted => {
                line += 1;
                field.push('\n');
            }
            c if c == sep && !quoted => record.push(std::mem::take(&mut field)),
            '\r' if !quoted => {}
            '\n' => {
                record.push(std::mem::take(&mut field));
                records.push((start, std::mem::take(&mut record)));
                line += 1;
                start = line;
            }
            c => field.push(c),
        }
    }
    if quoted {
        return Err(Error::at(start, "CSV: unterminated quoted field"));
    }
    if !field.is_empty() || !record.is_empty() {
        record.push(field);
        records.push((start, record));
    }
    records.retain(|(_, r)| r.iter().any(|f| !f.trim().is_empty()));
    let mut records = records.into_iter();
    let Some((_, header)) = records.next() else {
        return Err(Error::general("CSV: the file is empty"));
    };
    let header = header.into_iter().map(|h| h.trim().to_string()).collect();
    Ok(Table {
        header,
        rows: records.collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoting_and_separators() {
        let t = parse(
            "\u{feff}name,lat,lon\r\n\"Kew, \"\"Gardens\"\"\",51.48,-0.29\r\n\nBow,51.53,-0.02",
        )
        .unwrap();
        assert_eq!(t.header, ["name", "lat", "lon"]);
        assert_eq!(
            t.rows[0],
            (
                2,
                vec!["Kew, \"Gardens\"".into(), "51.48".into(), "-0.29".into()]
            )
        );
        assert_eq!(t.rows[1].0, 4);
        let t = parse("a;b\n1,5;2\n").unwrap();
        assert_eq!(t.rows[0].1, ["1,5", "2"]);
        assert!(parse("a,b\n\"open,1\n").is_err());
    }
}
