//! Choosing the fields to map and to name items by.

use super::read::{Feature, Prop};

const NAMES: &[&str] = &["name", "label", "title", "site", "station", "place"];
const IDS: &[&str] = &["id", "fid", "gid", "objectid", "ogc_fid", "osm_id", "code"];

/// Identifier-like fields are passed over when choosing what to map.
pub(crate) fn is_id(field: &str) -> bool {
    let f = field.to_ascii_lowercase();
    IDS.contains(&f.as_str())
        || ["_id", "_code", "id_str", "_cd"]
            .iter()
            .any(|s| f.ends_with(s))
}

/// The field that names each item: one of the usual names, or else any
/// text field whose name contains "name" or ends in "desc".
pub(crate) fn name_field(features: &[&Feature]) -> Option<String> {
    let text_fields: Vec<&String> = features
        .iter()
        .flat_map(|f| &f.props)
        .filter(|(_, p)| matches!(p, Prop::Text(_)))
        .map(|(k, _)| k)
        .collect();
    NAMES
        .iter()
        .find_map(|n| text_fields.iter().find(|k| k.eq_ignore_ascii_case(n)))
        .or_else(|| {
            text_fields.iter().find(|k| {
                let k = k.to_ascii_lowercase();
                k.contains("name") || k.ends_with("desc")
            })
        })
        .map(|k| (*k).clone())
}

/// Numeric fields in first-seen order: those with at least one number and
/// no text.
pub(crate) fn numeric_fields(features: &[&Feature]) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for f in features {
        for (k, _) in &f.props {
            if !names.contains(k) {
                names.push(k.clone());
            }
        }
    }
    names.retain(|n| {
        let (mut num, mut text) = (0, 0);
        for f in features {
            match f.props.iter().find(|(k, _)| k == n).map(|(_, p)| p) {
                Some(Prop::Number(_)) => num += 1,
                Some(Prop::Text(_)) => text += 1,
                Some(Prop::Missing) | None => {}
            }
        }
        num > 0 && text == 0
    });
    names
}
