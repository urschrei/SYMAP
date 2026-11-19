//! A CSV reader for the importer: a header row, then one record per line.
//! Commas, semicolons or tabs separate fields (whichever the header uses
//! most, with commas first on a tie); fields may be quoted, with `""` for a
//! quote. Lines end with LF, CRLF or CR.

use crate::Error;

#[derive(Debug)]
pub struct Table {
    pub header: Vec<String>,
    /// The line the header is on.
    pub header_line: usize,
    /// Each record with the line it starts on.
    pub rows: Vec<(usize, Vec<String>)>,
}

pub fn parse(text: &str) -> Result<Table, Error> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let first = text
        .split(['\n', '\r'])
        .find(|l| !l.trim().is_empty())
        .unwrap_or("");
    let sep = [',', ';', '\t']
        .into_iter()
        .enumerate()
        .max_by_key(|&(i, c)| (first.matches(c).count(), std::cmp::Reverse(i)))
        .map_or(',', |(_, c)| c);
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
            '\r' if !quoted && chars.peek() == Some(&'\n') => {}
            '\n' | '\r' if !quoted => {
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
    let Some((header_line, header)) = records.next() else {
        return Err(Error::general("CSV: the file is empty"));
    };
    let header = header.into_iter().map(|h| h.trim().to_string()).collect();
    Ok(Table {
        header,
        header_line,
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
        let e = parse("a,b\n\"open,1\n").unwrap_err();
        assert!(e.message.contains("unterminated"), "{e}");
    }

    #[test]
    fn blank_first_line_and_cr_line_ends() {
        let t = parse("\nx,y,v\n0,0,5\n").unwrap();
        assert_eq!((t.header_line, t.header.len()), (2, 3));
        let t = parse("x,y,v\r0,0,5\r1,1,2\r").unwrap();
        assert_eq!(t.header, ["x", "y", "v"]);
        assert_eq!(t.rows[1], (3, vec!["1".into(), "1".into(), "2".into()]));
        let t = parse("a,b\r\"x\ry\",2\r").unwrap();
        assert_eq!(t.rows[0].1, ["x\ry", "2"]);
        // A tie in separator counts goes to the comma.
        assert_eq!(parse("a,b;c\n1,2;3\n").unwrap().header, ["a", "b;c"]);
    }
}
