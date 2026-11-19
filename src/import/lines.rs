//! Lines (rivers, roads) from the data file or a second file: drawn on the
//! map as legends, or written as barriers.

use super::Sheet;
use super::fields::name_field;
use super::geometry::{clip, path};
use super::options::{LineUse, Options};
use super::project::Projection;
use super::read::{self, Feature, Geom, Prop, Source};
use super::topology::{self, P};
use crate::Error;
use crate::Point;

/// The lines to map, in source coordinates.
pub(crate) struct Lines {
    pub(crate) paths: Vec<Path>,
    /// How many line features there are (0 when they are left out).
    pub(crate) count: usize,
    /// Whether a lines file is in longitude and latitude.
    pub(crate) geographic: Option<bool>,
}

/// One part of a line feature.
pub(crate) struct Path {
    /// The feature's number, counting this file's lines first.
    feature: usize,
    name: Option<String>,
    points: Vec<P>,
}

/// Lines on the sheet, clipped a character beyond its edges.
pub(crate) struct Clipped {
    pub(crate) parts: Vec<Vec<Point>>,
    /// How many line features have a part on the sheet.
    pub(crate) drawn: usize,
    /// Their names, without repeats.
    pub(crate) names: Vec<String>,
}

/// Collect the lines of `features` (from `src`) and of the lines file.
pub(crate) fn gather(
    src: &Source,
    features: &[&Feature],
    opts: &Options,
    warnings: &mut Vec<String>,
) -> Result<Lines, Error> {
    let mut paths: Vec<Path> = Vec::new();
    for (k, f) in features.iter().enumerate() {
        if let Geom::Line(parts) = &f.geom {
            for r in parts {
                paths.push(Path {
                    feature: k,
                    name: line_name(f),
                    points: path(&src.arcs, r),
                });
            }
        }
    }
    let mut count = features.len();
    let mut geographic: Option<bool> = None;
    if let Some(t) = &opts.lines_text {
        let other = read::read(t, None)?;
        let n = other.features.len();
        let mut used = 0;
        let mut in_range = true;
        for f in &other.features {
            if let Geom::Line(parts) = &f.geom {
                let k = features.len() + used;
                used += 1;
                for r in parts {
                    let p = path(&other.arcs, r);
                    in_range &= p.iter().all(|q| q.0.abs() <= 180.0 && q.1.abs() <= 90.0);
                    paths.push(Path {
                        feature: k,
                        name: line_name(f),
                        points: p,
                    });
                }
            }
        }
        if used < n {
            warnings.push(format!(
                "{} features in the lines file that are not lines were left out",
                n - used
            ));
        }
        if used == 0 {
            return Err(Error::general("the lines file has no lines"));
        }
        count += used;
        geographic = Some(in_range);
    }
    if opts.lines == LineUse::Omit {
        if count > 0 {
            warnings.push(format!("{count} lines were left out"));
        }
        paths.clear();
        count = 0;
    }
    Ok(Lines {
        paths,
        count,
        geographic,
    })
}

/// A line feature's name, from a text field.
fn line_name(f: &Feature) -> Option<String> {
    name_field(&[f]).and_then(|k| {
        f.props
            .iter()
            .find(|(n, _)| *n == k)
            .and_then(|(_, p)| match p {
                Prop::Text(t) => Some(t.clone()),
                Prop::Number(_) | Prop::Missing => None,
            })
    })
}

/// Project the paths, simplify them to `tolerance`, and clip them to a
/// character beyond the sheet's edges.
pub(crate) fn clip_to(
    paths: &[Path],
    projection: &Projection,
    sheet: &Sheet,
    tolerance: f64,
) -> Clipped {
    let char_width = sheet.char_width;
    let mut parts: Vec<Vec<Point>> = Vec::new();
    let mut drawn: Vec<usize> = Vec::new();
    let mut names: Vec<String> = Vec::new();
    for line in paths {
        let projected: Vec<P> = line.points.iter().map(|&p| projection.project(p)).collect();
        let simple = topology::douglas_peucker(&projected, tolerance);
        let on_grid: Vec<Point> = simple.into_iter().map(|p| sheet.grid(p)).collect();
        let (min, max) = (
            Point::new(-char_width, -char_width),
            Point::new(
                sheet.down_extent + char_width,
                sheet.across_extent + char_width,
            ),
        );
        for part in clip(&on_grid, min, max) {
            parts.push(part);
            if !drawn.contains(&line.feature) {
                drawn.push(line.feature);
                if let Some(n) = line.name.as_ref().filter(|n| !names.contains(n)) {
                    names.push(n.clone());
                }
            }
        }
    }
    Clipped {
        parts,
        drawn: drawn.len(),
        names,
    }
}
