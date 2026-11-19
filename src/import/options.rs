//! The importer's options and its result.

use crate::Error;
use crate::MapKind;
use std::fmt;
use std::str::FromStr;

/// What to do with lines (LineString features).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineUse {
    /// Line legends (C-OTOLEGENDS), drawn on the map.
    Legend,
    /// Impermeable barriers to interpolation (D-BARRIERS).
    Barrier,
    Both,
    /// Left out.
    Omit,
}

impl LineUse {
    pub fn name(self) -> &'static str {
        match self {
            LineUse::Legend => "legend",
            LineUse::Barrier => "barrier",
            LineUse::Both => "both",
            LineUse::Omit => "none",
        }
    }

    pub(crate) fn draws(self) -> bool {
        match self {
            LineUse::Legend | LineUse::Both => true,
            LineUse::Barrier | LineUse::Omit => false,
        }
    }

    pub(crate) fn bars(self) -> bool {
        match self {
            LineUse::Barrier | LineUse::Both => true,
            LineUse::Legend | LineUse::Omit => false,
        }
    }
}

impl FromStr for LineUse {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Error> {
        match s {
            "legend" => Ok(LineUse::Legend),
            "barrier" => Ok(LineUse::Barrier),
            "both" => Ok(LineUse::Both),
            "none" => Ok(LineUse::Omit),
            _ => Err(Error::general(format!(
                "unknown lines '{s}': expected legend, barrier, both or none"
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Options {
    /// The property or column to map. If `None`, the first numeric one that
    /// is not an identifier, or else the first numeric one.
    pub value: Option<String>,
    /// Conformant for polygons and contour for points if `None`.
    pub map: Option<MapKind>,
    /// Printed width of the map in inches.
    pub width: f64,
    /// First title card; the source's name if `None`.
    pub title: Option<String>,
    /// Draw an outline: around points, their convex hull enlarged a little;
    /// for zones mapped as points, the zones themselves. Without one, the
    /// whole rectangle is mapped.
    pub outline: bool,
    /// The file's name, recorded in the deck's comments.
    pub source: Option<String>,
    /// The TopoJSON object to read; the first if `None`.
    pub object: Option<String>,
    /// What to do with lines, in the file or in `lines_text`.
    pub lines: LineUse,
    /// The line legends' symbol: up to four characters, struck over one
    /// another.
    pub line_symbol: String,
    /// Lines from a second file (GeoJSON or TopoJSON), and its name.
    pub lines_text: Option<String>,
    pub lines_source: Option<String>,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            value: None,
            map: None,
            width: 8.0,
            title: None,
            outline: true,
            source: None,
            object: None,
            lines: LineUse::Legend,
            line_symbol: "-".into(),
            lines_text: None,
            lines_source: None,
        }
    }
}

/// A deck made from imported data.
#[derive(Debug, Clone, PartialEq)]
pub struct Imported {
    pub deck: String,
    /// Properties or columns with numeric values, any of which can be mapped.
    pub fields: Vec<String>,
    /// The one mapped.
    pub value: String,
    pub map: MapKind,
    /// How many points or zones were mapped, and which.
    pub count: usize,
    pub unit: Unit,
    /// How many line features were used.
    pub lines: usize,
    pub warnings: Vec<String>,
}

/// What the importer maps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    Points,
    /// Polygons, as conformant zones or as points inside them.
    Zones,
}

impl Unit {
    pub fn name(self) -> &'static str {
        match self {
            Unit::Points => "points",
            Unit::Zones => "zones",
        }
    }
}

impl fmt::Display for Unit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}
