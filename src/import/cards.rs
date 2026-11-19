//! Writing the deck's cards and comments.

use super::lines::Clipped;
use super::options::LineUse;
use super::project::Projection;
use super::topology::P;
use super::{Item, Sheet, ZoneRing};
use crate::Error;
use crate::MapKind;
use std::fmt::Write;

/// The last part of a path.
pub(crate) fn file_name(path: &str) -> &str {
    path.rfind(['/', '\\']).map_or(path, |i| &path[i + 1..])
}

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
        .ok_or_else(|| Error::general(format!("{x:e} does not fit in a 10-column field")))
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

/// Everything the deck records.
pub(crate) struct Plan<'a> {
    /// The data file's name.
    pub(crate) source: &'a str,
    pub(crate) title: &'a str,
    /// "points" or "zones".
    pub(crate) unit: &'a str,
    /// The field mapped.
    pub(crate) value: &'a str,
    pub(crate) map: MapKind,
    pub(crate) zonal: bool,
    pub(crate) projection: &'a Projection,
    pub(crate) sheet: &'a Sheet,
    /// Simplification tolerance, half a character.
    pub(crate) tolerance: f64,
    /// What the lines are written as.
    pub(crate) written: LineUse,
    pub(crate) symbol: &'a str,
    pub(crate) lines_source: Option<&'a str>,
    pub(crate) clipped: &'a Clipped,
    pub(crate) warnings: &'a [String],
    /// Whether items have names to list.
    pub(crate) named: bool,
    pub(crate) items: &'a [Item],
    /// Zone rings with their zone numbers, on the sheet.
    pub(crate) rings: &'a [ZoneRing],
    pub(crate) outline: bool,
    pub(crate) point_outline: Option<&'a [P]>,
}

/// The deck: comments, then the packages and the F-MAP.
pub(crate) fn write_deck(plan: &Plan) -> Result<String, Error> {
    let Plan {
        source,
        title,
        unit,
        value,
        map,
        zonal,
        projection,
        sheet,
        tolerance,
        written,
        symbol,
        lines_source,
        clipped,
        warnings,
        named,
        items,
        rings,
        outline,
        point_outline,
    } = *plan;
    let (top, left, margin) = (sheet.top, sheet.left, sheet.margin);
    let n = items.len();
    let mut out = String::new();
    comment(
        &mut out,
        &format!("Made by symap import from {source}: {n} {unit}, mapping {value}."),
    );
    comment(&mut out, &projection.describe());
    comment(
        &mut out,
        &format!(
            "Locations are measured down from {top:.3} and across from {left:.3}, with a \
             margin of {margin:.3} around the {unit}."
        ),
    );
    if zonal {
        comment(
            &mut out,
            &format!(
                "Borders are simplified to within {tolerance:.3} (half a character), each \
                 shared border once, so neighbouring zones still meet."
            ),
        );
        if map != MapKind::Conformant {
            comment(
                &mut out,
                "Each zone's value is placed at its centroid as a data point.",
            );
        }
    }
    let how = match written {
        LineUse::Both => Some(format!(
            "drawn as line legends ({symbol}) and written as impermeable barriers (strength -1)"
        )),
        LineUse::Legend => Some(format!("drawn as line legends ({symbol})")),
        LineUse::Barrier => Some("written as impermeable barriers (strength -1)".to_string()),
        LineUse::Omit => None,
    };
    if let Some(how) = how {
        let from = lines_source.map_or(String::new(), |s| format!(" from {}", file_name(s)));
        let named = if clipped.names.is_empty() {
            String::new()
        } else {
            format!(" ({})", clipped.names.join(", "))
        };
        comment(
            &mut out,
            &format!(
                "{} lines{from}{named} are {how}, simplified to within {tolerance:.3} \
                 and cut a character beyond the map's edges.",
                clipped.drawn
            ),
        );
    }
    for w in warnings {
        comment(&mut out, &format!("Note: {w}."));
    }
    if named {
        let what = if zonal { "Zones" } else { "Data points" };
        let _ = writeln!(out, "#\n# {what}, in order:");
        for (i, item) in items.iter().enumerate() {
            if let Some(name) = &item.name {
                let name: String = name.chars().filter(|c| !c.is_control()).take(70).collect();
                let _ = writeln!(out, "# {:>4} {name}", i + 1);
            }
        }
    }

    let write_ring = |out: &mut String, head: &str, ring: &[P]| -> Result<(), Error> {
        for (j, &p) in ring.iter().enumerate() {
            let _ = writeln!(
                out,
                "{}",
                coordinate_card(if j == 0 { head } else { "" }, p)?
            );
        }
        Ok(())
    };
    if map == MapKind::Conformant {
        let _ = writeln!(out, "A-CONFORMOLINES");
        for (z, ring) in rings {
            write_ring(&mut out, &format!("{z:>5}    A"), ring)?;
        }
        let _ = writeln!(out, "99999");
    } else {
        if outline {
            if zonal {
                let _ = writeln!(out, "A-OUTLINE");
                for (_, ring) in rings {
                    write_ring(&mut out, "", ring)?;
                }
                let _ = writeln!(out, "99999");
            } else if let Some(ring) = point_outline {
                let _ = writeln!(out, "A-OUTLINE");
                write_ring(&mut out, "", ring)?;
                let _ = writeln!(out, "99999");
            }
        }
        let _ = writeln!(out, "B-DATA POINTS");
        for i in items {
            let _ = writeln!(out, "{}", coordinate_card("", i.point)?);
        }
        let _ = writeln!(out, "99999");
    }
    if written.draws() {
        let _ = writeln!(out, "C-OTOLEGENDS");
        for part in &clipped.parts {
            write_ring(&mut out, &format!("     {symbol:<4}L"), part)?;
        }
        let _ = writeln!(out, "99999");
    }
    if written.bars() {
        // Repeating a barrier's last vertex ends it.
        let _ = writeln!(out, "D-BARRIERS");
        for part in &clipped.parts {
            for &p in part.iter().chain(part.last()) {
                let _ = writeln!(out, "{}{}", coordinate_card("", p)?, field_with(-1.0, 1)?);
            }
        }
        let _ = writeln!(out, "99999");
    }
    let _ = writeln!(out, "E-VALUES");
    for i in items {
        let _ = writeln!(out, "{:10}{}", "", field(i.value)?);
    }
    let _ = writeln!(out, "99999");

    let _ = writeln!(out, "F-MAP");
    let _ = writeln!(out, "{}", card_text(title));
    let counted = if zonal {
        format!("{value} for {n} zones")
    } else {
        format!("{value} at {n} points")
    };
    let _ = writeln!(out, "{}", card_text(&counted));
    let _ = writeln!(
        out,
        "{}",
        match map {
            MapKind::Contour => "CONTOUR MAP - SHEPARD INTERPOLATION",
            MapKind::Proximal => "PROXIMAL MAP - NEAREST DATA POINT",
            MapKind::Conformant => "CONFORMANT MAP",
        }
    );
    let _ = writeln!(
        out,
        "    1{:5}{}{}",
        "",
        field_with(sheet.height, 2)?,
        field_with(sheet.width, 2)?
    );
    let _ = writeln!(
        out,
        "    2{:5}{}{}{}{}",
        "",
        field_with(0.0, 3)?,
        field_with(0.0, 3)?,
        field_with(sheet.down_extent, 3)?,
        field_with(sheet.across_extent, 3)?
    );
    if map == MapKind::Proximal {
        let _ = writeln!(out, "   31\n   36\n   37");
    }
    let _ = writeln!(out, "99999\n999999");
    Ok(out)
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
        assert!(
            field(1e12)
                .unwrap_err()
                .message
                .starts_with("1e12 does not fit")
        );
    }

    #[test]
    fn file_names() {
        assert_eq!(file_name("data/a/no2.csv"), "no2.csv");
        assert_eq!(file_name("C:\\data\\no2.csv"), "no2.csv");
        assert_eq!(file_name("no2.csv"), "no2.csv");
    }
}
