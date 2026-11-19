use std::io::Read;
use std::process::ExitCode;

const USAGE: &str = "usage: symap [--asa | --values] [DECK | -]\n\
    \x20      symap import [OPTIONS] [FILE | -]\n\n\
    Reads a SYMAP deck (from stdin if DECK is omitted or '-') and prints its maps.\n\
    --asa     emit Fortran carriage control with overprint lines instead of plain text\n\
    --values  print the computed values of maps with elective 21, as CSV\n\n\
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

fn read_input(path: Option<&str>) -> std::io::Result<String> {
    match path {
        None | Some("-") => {
            let mut s = String::new();
            std::io::stdin().read_to_string(&mut s).map(|_| s)
        }
        Some(p) => std::fs::read_to_string(p),
    }
}

/// Read `symap import`'s options into `opts`; returns the input path and
/// the lines file.
fn import_args(
    mut p: lexopt::Parser,
    opts: &mut symap::import::Options,
) -> Result<(Option<String>, Option<String>), lexopt::Error> {
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
            Long("lines-from") => lines_from = Some(p.value()?.string()?),
            Long("line-symbol") => opts.line_symbol = p.value()?.string()?,
            Long("lines") => {
                let v = p.value()?.string()?;
                opts.lines = v.parse::<LineUse>().map_err(|e| format!("--lines: {e}"))?;
            }
            Long("contour") => opts.map = Some(MapKind::Contour),
            Long("proximal") => opts.map = Some(MapKind::Proximal),
            Long("conformant") => opts.map = Some(MapKind::Conformant),
            Long("no-outline") => opts.outline = false,
            Value(v) if path.is_none() => path = Some(v.string()?),
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
    opts.source = path.clone().filter(|p| p != "-");
    if let Some(f) = &lines_from {
        match std::fs::read_to_string(f) {
            Ok(t) => {
                opts.lines_text = Some(t);
                opts.lines_source = Some(f.clone());
            }
            Err(e) => {
                eprintln!("symap import: {f}: {e}");
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
            print!("{}", imp.deck);
            ExitCode::SUCCESS
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

/// The options for printing a deck: the output and the deck's path.
fn run_args(mut p: lexopt::Parser) -> Result<(Output, Option<String>), lexopt::Error> {
    use lexopt::prelude::*;
    let (mut output, mut path) = (Output::Text, None);
    while let Some(arg) = p.next()? {
        let mode = match arg {
            Long("asa") => Output::Asa,
            Long("values") => Output::Values,
            Value(v) if path.is_none() => {
                path = Some(v.string()?);
                continue;
            }
            _ => return Err(arg.unexpected()),
        };
        if output != Output::Text && output != mode {
            return Err("--asa and --values cannot be used together".into());
        }
        output = mode;
    }
    Ok((output, path))
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
    let (output, path) = match run_args(lexopt::Parser::from_args(args)) {
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
            match output {
                Output::Values => {
                    let csv: Vec<String> = run.maps.iter().filter_map(|m| m.values_csv()).collect();
                    if csv.is_empty() {
                        eprintln!("symap: no map in this deck uses elective 21 (store values)");
                        return ExitCode::FAILURE;
                    }
                    print!("{}", csv.join("\n"));
                }
                Output::Asa => {
                    for m in &run.maps {
                        print!("{}", m.to_asa());
                    }
                }
                Output::Text => {
                    for (i, m) in run.maps.iter().enumerate() {
                        if i > 0 {
                            println!("\x0c");
                        }
                        print!("{}", m.to_text());
                    }
                }
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("symap: {e}");
            ExitCode::FAILURE
        }
    }
}
