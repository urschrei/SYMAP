use symap::{Level, MapKind, Strikes, run};

fn deck(name: &str) -> String {
    std::fs::read_to_string(format!(
        "{}/examples/decks/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

/// A deck of four points on a square, with extra F-MAP cards.
fn square(values: [&str; 4], electives: &str) -> String {
    let pts = ["0 0", "0 10", "10 0", "10 10"].join("\n");
    format!(
        "B-DATA POINTS\n{pts}\n99999\nE-VALUES\n{}\n99999\nF-MAP\nT1\nT2\nT3\n    1       2.0       2.0\n    2      -1.0      -1.0      11.0      11.0\n{electives}99999\n999999\n",
        values.join("\n")
    )
}

#[test]
fn manual_example_1_14() {
    let r = run(&deck("example-1-14.sym")).unwrap();
    assert!(r.diagnostics.is_empty(), "{:?}", r.diagnostics);
    let (a, b) = (&r.maps[0], &r.maps[1]);
    assert_eq!((a.kind, b.kind), (MapKind::Proximal, MapKind::Proximal));
    // Elective 1: 5 x 8 inches at 8 lines and 10 columns per inch.
    assert_eq!((a.frame.rows, a.frame.cols), (40, 80));
    // As printed in Example 1-17: five equal levels, 1 1 1 1 2 points.
    assert_eq!(a.classes.breaks, vec![10.0, 20.0, 30.0, 40.0, 50.0, 60.0]);
    assert_eq!(a.frequency.levels, vec![1, 1, 1, 1, 2]);
    let text = a.to_text();
    assert!(text.contains("A = AIRPORT") && text.contains("I----+----I"));
    // The second map: legends deleted, dashed background, black boundaries, values printed.
    let text = b.to_text();
    assert!(!text.contains("AIRPORT"));
    assert_eq!(b.at(1, 1), Strikes::new("-"));
    assert!(b.grid.contains(&Strikes::new("OXAV")));
    for v in ["10.00", "20.00", "30.00", "40.00", "50.00", "60.00"] {
        assert!(text.contains(v), "{v}");
    }
}

#[test]
fn hills_contour_and_repeated_electives() {
    let r = run(&deck("hills.sym")).unwrap();
    let (c, p) = (&r.maps[0], &r.maps[1]);
    assert_eq!((c.kind, p.kind), (MapKind::Contour, MapKind::Proximal));
    // Elective 12 carried electives 1 and 3 into the second map.
    assert_eq!(p.classes.levels(), 6);
    assert_eq!((p.frame.rows, p.frame.cols), (c.frame.rows, c.frame.cols));
    // The surface stays within the data (electives 32 and 33 default to its extremes).
    assert_eq!(c.frequency.low + c.frequency.high, 0);
    assert!(c.levels.iter().flatten().all(|l| matches!(l, Level::In(_))));
    // White contour lines: blank characters inside the study area.
    let w = c.width();
    let lined = (0..c.grid.len())
        .filter(|&i| c.levels[i].is_some() && c.grid[i].is_blank())
        .count();
    assert!(lined > 50, "{lined}");
    assert!(c.levels[w + 1].is_none(), "corner is outside the outline");
}

#[test]
fn districts_conformant() {
    let r = run(&deck("districts.sym")).unwrap();
    let m = &r.maps[0];
    assert_eq!(m.kind, MapKind::Conformant);
    assert_eq!(m.frequency.levels.iter().sum::<usize>(), 6);
}

#[test]
fn value_range_electives_give_low_and_high() {
    let r = run(&square(
        ["0", "5", "10", "20"],
        "    4       2.0\n    5      15.0\n    8\n",
    ))
    .unwrap();
    let m = &r.maps[0];
    assert_eq!((m.frequency.low, m.frequency.high), (1, 1));
    let text = m.to_text();
    assert!(text.contains("BELOW") && text.contains("ABOVE"));
    assert!(m.grid.contains(&Strikes::new("L.")) && m.grid.contains(&Strikes::new("HHH/")));
}

#[test]
fn equal_count_levels() {
    let r = run(&square(["1", "2", "3", "100"], "    3       2.\n    6\n")).unwrap();
    assert_eq!(r.maps[0].frequency.levels, vec![2, 2]);
}

#[test]
fn invalid_and_superimposed_points() {
    let d = "B-DATA POINTS\n0 0\n0 10\n10 0\n10 10\n10 10\n99999\nE-VALUES\n1\n\n3\n4\n4\n99999\n\
             F-MAP\nT\n\n\n    1       2.0       2.0\n    2      -1.0      -1.0      11.0      11.0\n   18\n99999\n999999\n";
    let m = &run(d).unwrap().maps[0];
    assert!(
        m.grid.contains(&Strikes::new("M")),
        "blank value is invalid under elective 18"
    );
    assert!(
        m.grid.contains(&Strikes::new("S")),
        "two points in one character"
    );
    assert_eq!(m.frequency.levels.iter().sum::<usize>(), 4);
}

#[test]
fn wide_maps_print_in_panels() {
    let m = &run(&square(["1", "2", "3", "4"], "    1       2.0      30.0\n"))
        .unwrap()
        .maps[0];
    assert_eq!(m.frame.cols, 300);
    assert_eq!(m.panels().len(), 3);
}

#[test]
fn text_and_asa_agree() {
    let m = &run(&deck("hills.sym")).unwrap().maps[0];
    let (text, asa) = (m.to_text(), m.to_asa());
    assert_eq!(
        asa.lines().filter(|l| !l.starts_with('+')).count(),
        text.lines().count()
    );
    assert!(asa.starts_with('1'));
    assert!(
        asa.lines().any(|l| l.starts_with('+')),
        "dark levels overprint"
    );
}

#[test]
fn errors() {
    let short = "B-DATA POINTS\n0 0\n1 1\n99999\nE-VALUES\n5\n99999\nF-MAP\nT\n\n\n99999\n999999\n";
    let e = run(short).unwrap_err();
    assert_eq!(e.line, Some(8));
    assert!(e.message.contains("E-VALUES has only 1"));
    assert!(run("B-DATA POINTS\n0 0\n99999\n999999\n").is_err());
    assert!(run(&square(["1", "2", "3", "4"], "    3      12.\n")).is_err());
}

#[test]
fn manual_example_1_3_contour() {
    // Row-and-column measurement; Example 1-5 is its printed output.
    let r = run(&deck("example-1-3.sym")).unwrap();
    let m = &r.maps[0];
    assert_eq!(m.kind, MapKind::Contour);
    assert_eq!((m.frame.rows, m.frame.cols), (40, 80));
    // As printed on p. 13: extremes, level ranges and frequencies.
    assert_eq!(m.data_range, (-14.0, 43.0));
    let want = [-14.0, -2.6, 8.8, 20.2, 31.6, 43.0];
    for (a, b) in m.classes.breaks.iter().zip(want) {
        assert!((a - b).abs() < 1e-9, "{a} {b}");
    }
    assert_eq!(m.frequency.levels, vec![2, 1, 1, 0, 2]);
    // Histogram bars: two rows, as in the original.
    let text = m.to_text();
    assert!(text.contains("1   I..1..I    I++2++I    IOO3OOI               I██5██I"));
    assert!(text.contains("2   I..1..I                                     I██5██I"));
    // Elective 9 removes only the bars.
    let no_bars =
        deck("example-1-3.sym").replace("    2            0.0", "    9\n    2            0.0");
    let text = run(&no_bars).unwrap().maps[0].to_text();
    assert!(text.contains("FREQ.") && !text.contains("I..1..I"));
}
