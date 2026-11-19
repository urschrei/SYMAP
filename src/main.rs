use std::io::Read;
use std::process::ExitCode;

const USAGE: &str = "usage: symap [--asa | --values] [DECK | -]\n\
    \x20      symap import [OPTIONS] [FILE | -]\n\n\
    Reads a SYMAP deck (from stdin if DECK is omitted or '-') and prints its maps.\n\
    --asa     emit Fortran carriage control with overprint lines instead of plain text\n\
    --values  print the computed values of maps with elective 21, as CSV\n\n\
    import reads points (CSV, GeoJSON, TopoJSON) or polygons (GeoJSON, TopoJSON)\n\
    and prints a deck that maps them.\n\
    --value FIELD   the column or property to map (the first numeric one)\n\
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

fn import(args: &[String]) -> ExitCode {
    let mut opts = symap::import::Options::default();
    let mut path: Option<String> = None;
    let mut lines_from: Option<String> = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut next = |name: &str| {
            it.next()
                .cloned()
                .ok_or_else(|| format!("{name} needs a value"))
        };
        let r: Result<(), String> = match a.as_str() {
            "--value" => next("--value").map(|v| opts.value = Some(v)),
            "--title" => next("--title").map(|v| opts.title = Some(v)),
            "--width" => next("--width").and_then(|v| {
                v.parse()
                    .map(|w| opts.width = w)
                    .map_err(|_| format!("--width: '{v}' is not a number"))
            }),
            "--object" => next("--object").map(|v| opts.object = Some(v)),
            "--lines-from" => next("--lines-from").map(|v| lines_from = Some(v)),
            "--line-symbol" => next("--line-symbol").map(|v| opts.line_symbol = v),
            "--lines" => next("--lines").and_then(|v| {
                symap::import::LineUse::parse(&v)
                    .map(|l| opts.lines = l)
                    .ok_or_else(|| format!("--lines: '{v}' is not legend, barrier, both or none"))
            }),
            "--contour" | "--proximal" | "--conformant" => {
                opts.map = symap::import::MapType::parse(&a[2..]);
                Ok(())
            }
            "--no-outline" => {
                opts.outline = false;
                Ok(())
            }
            _ if path.is_none() && !a.starts_with("--") => {
                path = Some(a.clone());
                Ok(())
            }
            _ => Err(format!("unexpected argument '{a}'")),
        };
        if let Err(e) = r {
            eprintln!("symap import: {e}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    }
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

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("import") {
        return import(&args[1..]);
    }
    let mut asa = false;
    let mut values = false;
    let mut path: Option<String> = None;
    for a in args {
        match a.as_str() {
            "--asa" => asa = true,
            "--values" => values = true,
            "-h" | "--help" => {
                println!("{USAGE}");
                return ExitCode::SUCCESS;
            }
            _ if path.is_none() => path = Some(a),
            _ => {
                eprintln!("{USAGE}");
                return ExitCode::from(2);
            }
        }
    }

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
            if values {
                let csv: Vec<String> = run.maps.iter().filter_map(|m| m.values_csv()).collect();
                if csv.is_empty() {
                    eprintln!("symap: no map in this deck uses elective 21 (store values)");
                    return ExitCode::FAILURE;
                }
                print!("{}", csv.join("\n"));
                return ExitCode::SUCCESS;
            }
            for (i, m) in run.maps.iter().enumerate() {
                for w in &m.warnings {
                    eprintln!("warning: {w}");
                }
                if asa {
                    print!("{}", m.to_asa());
                } else {
                    if i > 0 {
                        println!("\x0c");
                    }
                    print!("{}", m.to_text());
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
