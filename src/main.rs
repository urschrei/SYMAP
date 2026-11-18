use std::io::Read;
use std::process::ExitCode;

const USAGE: &str = "usage: symap [--asa | --values] [DECK | -]\n\n\
    Reads a SYMAP deck (from stdin if DECK is omitted or '-') and prints its maps.\n\
    --asa     emit Fortran carriage control with overprint lines instead of plain text\n\
    --values  print the computed values of maps with elective 21, as CSV";

fn main() -> ExitCode {
    let mut asa = false;
    let mut values = false;
    let mut path: Option<String> = None;
    for a in std::env::args().skip(1) {
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

    let text = match path.as_deref() {
        None | Some("-") => {
            let mut s = String::new();
            std::io::stdin().read_to_string(&mut s).map(|_| s)
        }
        Some(p) => std::fs::read_to_string(p),
    };
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
