use std::io::{ErrorKind, Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const USAGE: &str = "usage: symap [--asa | --values] [--color WHEN] [DECK | -]\n\
    \x20      symap import [OPTIONS] [FILE | -]\n\n\
    Reads a SYMAP deck (from stdin if DECK is omitted or '-') and prints its maps.\n\
    --asa     emit Fortran carriage control with overprint lines instead of plain text\n\
    --values  print the computed values of maps with elective 21, as CSV\n\
    --color WHEN  shade overprinted symbols' paper by their ink: auto (the\n\
    \x20             default: on a terminal, unless NO_COLOR is set), always\n\
    \x20             or never. COLORTERM=truecolor gives 24-bit greys,\n\
    \x20             otherwise the 256-colour palette's\n\n\
    import reads points (CSV, GeoJSON, TopoJSON) or polygons (GeoJSON, TopoJSON)\n\
    and prints a deck that maps them.\n\
    --value FIELD   the column or property to map (the first numeric one\n\
    \x20               that is not an identifier)\n\
    --conformant    a conformant map of the polygons (the default for polygons)\n\
    --contour       a contour map (the default for points; zones give centroids)\n\
    --proximal      a proximal map\n\
    --object NAME   the TopoJSON object to read (the first)\n\
    --lines-from F  lines (GeoJSON or TopoJSON) to add from a second file\n\
    --lines USE     legend (the default), barrier, both or none\n\
    --line-symbol S the line legends' symbol, up to four characters (-)\n\
    --width INCHES  printed width of the map (8)\n\
    --title TEXT    first title card (the file's name)\n\
    --no-outline    map the whole rectangle, not just around the points";

/// The file at `path`, or standard input if it is `None` or `-`.
fn read_input(path: Option<&Path>) -> std::io::Result<String> {
    match path.filter(|p| *p != Path::new("-")) {
        None => {
            let mut s = String::new();
            std::io::stdin().read_to_string(&mut s).map(|_| s)
        }
        Some(p) => std::fs::read_to_string(p),
    }
}

/// Write `text` to standard output. A closed pipe (as from `head`) is not
/// a failure.
fn emit(text: &str) -> ExitCode {
    match std::io::stdout().lock().write_all(text.as_bytes()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) if e.kind() == ErrorKind::BrokenPipe => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("symap: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Read `symap import`'s options into `opts`; returns the input path and
/// the lines file.
fn import_args(
    mut p: lexopt::Parser,
    opts: &mut symap::import::Options,
) -> Result<(Option<PathBuf>, Option<PathBuf>), lexopt::Error> {
    use lexopt::prelude::*;
    use symap::MapKind;
    use symap::import::LineUse;
    let (mut path, mut lines_from) = (None, None);
    while let Some(arg) = p.next()? {
        match arg {
            Long("value") => opts.value = Some(p.value()?.string()?),
            Long("title") => opts.title = Some(p.value()?.string()?),
            Long("width") => opts.width = p.value()?.parse()?,
            Long("object") => opts.object = Some(p.value()?.string()?),
            Long("lines-from") => lines_from = Some(PathBuf::from(p.value()?)),
            Long("line-symbol") => opts.line_symbol = p.value()?.string()?,
            Long("lines") => {
                let v = p.value()?.string()?;
                opts.lines = v.parse::<LineUse>().map_err(|e| format!("--lines: {e}"))?;
            }
            Long("contour") => opts.map = Some(MapKind::Contour),
            Long("proximal") => opts.map = Some(MapKind::Proximal),
            Long("conformant") => opts.map = Some(MapKind::Conformant),
            Long("no-outline") => opts.outline = false,
            Value(v) if path.is_none() => path = Some(PathBuf::from(v)),
            _ => return Err(arg.unexpected()),
        }
    }
    Ok((path, lines_from))
}

fn import(args: lexopt::Parser) -> ExitCode {
    let mut opts = symap::import::Options::default();
    let (path, lines_from) = match import_args(args, &mut opts) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("symap import: {e}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    opts.source = path
        .as_deref()
        .filter(|p| *p != Path::new("-"))
        .map(|p| p.to_string_lossy().into_owned());
    if let Some(f) = &lines_from {
        match std::fs::read_to_string(f) {
            Ok(t) => {
                opts.lines_text = Some(t);
                opts.lines_source = Some(f.to_string_lossy().into_owned());
            }
            Err(e) => {
                eprintln!("symap import: {}: {e}", f.display());
                return ExitCode::FAILURE;
            }
        }
    }
    let text = match read_input(path.as_deref()) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("symap import: {e}");
            return ExitCode::FAILURE;
        }
    };
    match symap::import::import(&text, &opts) {
        Ok(imp) => {
            for w in &imp.warnings {
                eprintln!("warning: {w}");
            }
            eprintln!(
                "{} map of {} for {} {} (numeric fields: {})",
                imp.map.name(),
                imp.value,
                imp.count,
                imp.unit,
                imp.fields.join(", ")
            );
            emit(&imp.deck)
        }
        Err(e) => {
            eprintln!("symap import: {e}");
            ExitCode::FAILURE
        }
    }
}

/// What to print for a deck.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Output {
    /// Plain text, one character per print position.
    Text,
    /// Fortran carriage control (`--asa`).
    Asa,
    /// The values of maps with elective 21, as CSV (`--values`).
    Values,
}

/// When to print in colour (`--color`).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Color {
    Auto,
    Always,
    Never,
}

/// The palette for plain-text output, if any: `--color`, then whether
/// standard output is a terminal, `NO_COLOR` (https://no-color.org),
/// `TERM=dumb` and `COLORTERM`.
fn palette(color: Color) -> Option<symap::Palette> {
    use std::io::IsTerminal;
    let var = |k| std::env::var_os(k).filter(|v| !v.is_empty());
    let on = match color {
        Color::Never => false,
        Color::Always => true,
        Color::Auto => {
            std::io::stdout().is_terminal()
                && var("NO_COLOR").is_none()
                && var("TERM").is_none_or(|t| t != "dumb")
        }
    };
    let truecolor = var("COLORTERM").is_some_and(|v| v == "truecolor" || v == "24bit");
    on.then_some(if truecolor {
        symap::Palette::TrueColor
    } else {
        symap::Palette::Ansi256
    })
}

/// The options for printing a deck: the output, colour and the deck's path.
fn run_args(mut p: lexopt::Parser) -> Result<(Output, Color, Option<PathBuf>), lexopt::Error> {
    use lexopt::prelude::*;
    let (mut output, mut color, mut path) = (Output::Text, Color::Auto, None);
    while let Some(arg) = p.next()? {
        let mode = match arg {
            Long("asa") => Output::Asa,
            Long("values") => Output::Values,
            Long("color") | Long("colour") => {
                color = match p.value()?.string()?.as_str() {
                    "auto" => Color::Auto,
                    "always" => Color::Always,
                    "never" => Color::Never,
                    v => {
                        return Err(
                            format!("--color: expected auto, always or never, not '{v}'").into(),
                        );
                    }
                };
                continue;
            }
            Value(v) if path.is_none() => {
                path = Some(PathBuf::from(v));
                continue;
            }
            _ => return Err(arg.unexpected()),
        };
        if output != Output::Text && output != mode {
            return Err("--asa and --values cannot be used together".into());
        }
        output = mode;
    }
    Ok((output, color, path))
}

fn main() -> ExitCode {
    let mut args: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    if args.first().is_some_and(|a| a == "-h" || a == "--help") {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    if args.first().is_some_and(|a| a == "import") {
        args.remove(0);
        return import(lexopt::Parser::from_args(args));
    }
    let (output, color, path) = match run_args(lexopt::Parser::from_args(args)) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("symap: {e}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };

    let text = read_input(path.as_deref());
    let text = match text {
        Ok(t) => t,
        Err(e) => {
            eprintln!("symap: {e}");
            return ExitCode::FAILURE;
        }
    };

    match symap::run(&text) {
        Ok(run) => {
            for d in &run.diagnostics {
                eprintln!("warning: {d}");
            }
            for w in run.maps.iter().flat_map(|m| &m.warnings) {
                eprintln!("warning: {w}");
            }
            let text = match output {
                Output::Values => {
                    let csv: Vec<String> = run.maps.iter().filter_map(|m| m.values_csv()).collect();
                    if csv.is_empty() {
                        eprintln!("symap: no map in this deck uses elective 21 (store values)");
                        return ExitCode::FAILURE;
                    }
                    csv.join("\n")
                }
                Output::Asa => run.maps.iter().map(|m| m.to_asa()).collect(),
                Output::Text => {
                    let palette = palette(color);
                    let pages: Vec<String> = run
                        .maps
                        .iter()
                        .map(|m| match palette {
                            Some(p) => m.to_ansi(p),
                            None => m.to_text(),
                        })
                        .collect();
                    pages.join("\x0c\n")
                }
            };
            emit(&text)
        }
        Err(e) => {
            eprintln!("symap: {e}");
            ExitCode::FAILURE
        }
    }
}
