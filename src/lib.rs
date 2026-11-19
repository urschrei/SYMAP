//! An implementation of SYMAP, the Synagraphic Mapping System (Howard
//! Fisher and Betty Benson; Harvard Laboratory for Computer Graphics and
//! Spatial Analysis), following the 1971 program manual.
//!
//! It reads SYMAP card decks and prints line-printer maps: contour (Shepard
//! interpolation), proximal (nearest data point) and conformant (zone fill),
//! with overprinted tone symbols, white contour lines, legends, and the
//! numeric interpretation and frequency chart below the map.
//!
//! ```
//! let deck = "B-DATA POINTS\n0 0\n0 10\n10 0\n10 10\n99999\n\
//!             E-VALUES\n1\n2\n3\n4\n99999\n\
//!             F-MAP\nEXAMPLE\n\n\n    1       2.0       2.0\n99999\n999999\n";
//! let run = symap::run(deck).unwrap();
//! print!("{}", run.maps[0].to_text());
//! ```

pub mod card;
pub mod classify;
pub mod deck;
pub mod elective;
pub mod error;
pub mod frame;
pub mod geom;
pub mod import;
pub mod interp;
pub mod json;
pub mod map;
pub mod page;
pub mod symbols;
#[cfg(target_arch = "wasm32")]
pub mod wasm;

pub use classify::{Classes, Level};
pub use deck::{Deck, MapKind, MapSpec};
pub use elective::Elective;
pub use error::{Diagnostic, Error};
pub use frame::Frame;
pub use geom::Point;
pub use map::PrintedMap;
pub use page::Line;
pub use symbols::{Strikes, Symbolism};

/// The result of running a deck.
#[derive(Debug, Clone, PartialEq)]
pub struct Run {
    pub maps: Vec<PrintedMap>,
    pub diagnostics: Vec<Diagnostic>,
}

/// Parse a deck and draw every map it requests.
pub fn run(text: &str) -> Result<Run, Error> {
    let deck = Deck::parse(text)?;
    if deck.maps.is_empty() {
        return Err(Error::general("deck contains no F-MAP package"));
    }
    let maps = deck.maps.iter().map(map::draw).collect::<Result<_, _>>()?;
    Ok(Run {
        maps,
        diagnostics: deck.diagnostics,
    })
}
