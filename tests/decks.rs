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
    assert_eq!(m.panels(), vec![0..=130, 130..=260, 260..=300]);
    let narrow = &run(&deck("hills.sym")).unwrap().maps[0];
    assert_eq!(narrow.panels(), vec![0..=narrow.frame.cols]);
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

#[test]
fn manual_example_1_20_conformant_boundaries() {
    // Cells read from the printed map, Example 1-22 (manual p. 42).
    let r = run(&deck("example-1-20.sym")).unwrap();
    let m = &r.maps[0];
    assert_eq!(m.kind, MapKind::Conformant);
    let at = |r: usize, c: usize| m.at(r, c);
    let plus = Strikes::new("+");
    let dot = Strikes::new(".");
    let solid = Strikes::new("OXAV");
    // The zone 1|2 edge runs along column 25: the line sits on the edge.
    for row in [12, 16] {
        assert_eq!(
            (at(row, 24), at(row, 25), at(row, 26)),
            (plus, Strikes::BLANK, dot),
            "row {row}"
        );
    }
    // A shallow edge (zones 4|5) blanks the characters it passes along.
    assert_eq!(at(25, 51), solid);
    assert!((52..=56).all(|c| at(25, c).is_blank()));
    // Zone 4's outer edge is not a boundary between zones: still filled.
    assert_eq!(at(25, 57), plus);
    // Zones 3 and 5 share a level but are still separated by a line.
    assert!(at(25, 45).is_blank() && at(25, 44) == solid && at(25, 46) == solid);
}

#[test]
fn barriers_stop_interpolation() {
    let r = run(&deck("barrier.sym")).unwrap();
    assert!(
        r.maps.iter().all(|m| m.warnings.is_empty()),
        "{:?}",
        r.maps[0].warnings
    );
    let (with, without) = (&r.maps[0], &r.maps[1]);
    // Across the river on line 20: with the barrier the surface jumps from the
    // top level straight to the bottom; without it, it passes through between.
    let levels = |m: &symap::PrintedMap| {
        let mut v: Vec<Level> = (30..50).filter_map(|c| m.level(20, c)).collect();
        v.sort();
        v.dedup();
        v
    };
    assert_eq!(levels(with), vec![Level::In(0), Level::In(4)]);
    assert!(levels(without).len() >= 4, "{:?}", levels(without));
}

#[test]
fn tables_before_the_map_match_the_manual() {
    // Example 1-4 (data points) and Example 1-21 (zones), as printed.
    let m = &run(&deck("example-1-3.sym")).unwrap().maps[0];
    let rows: Vec<(i64, i64, i64)> = m
        .table
        .iter()
        .map(|t| (t.row, t.col, (t.value.unwrap() * 100.0).round() as i64))
        .collect();
    assert_eq!(
        rows,
        vec![
            (24, 13, 1200),
            (13, 34, -1400),
            (26, 30, 4300),
            (20, 53, 3800),
            (28, 48, -400),
            (30, 70, 0)
        ]
    );
    let text = m.to_text();
    assert!(text.starts_with("MAP SCALE =     1.0000 INCHES ON OUTPUT MAP/UNITS ON SOURCE MAP\n"));
    assert!(text.contains("    6)                30        70         6         0.0         2\n"));
    // Example 1-4 prints the standard search radius.
    assert!(text.contains("STANDARD SEARCH RADIUS IS    3.92710\n"));
    let z = &run(&deck("example-1-20.sym")).unwrap().maps[0];
    let zones: Vec<(i64, i64, usize)> = z
        .table
        .iter()
        .map(|t| {
            (
                t.row,
                t.col,
                match t.level {
                    Some(Level::In(k)) => k as usize + 1,
                    _ => 0,
                },
            )
        })
        .collect();
    assert_eq!(
        zones,
        vec![
            (20, 17, 2),
            (13, 36, 1),
            (27, 34, 5),
            (19, 52, 2),
            (29, 49, 5),
            (30, 70, 4)
        ]
    );
    // Elective 17 suppresses the table.
    let quiet =
        deck("example-1-3.sym").replace("    2            0.0", "   17\n    2            0.0");
    assert!(run(&quiet).unwrap().maps[0].to_text().starts_with("*----"));
}

#[test]
fn elective_21_keeps_values() {
    let d = deck("example-1-3.sym").replace("    2            0.0", "   21\n    2            0.0");
    let m = &run(&d).unwrap().maps[0];
    let csv = m.values_csv().unwrap();
    let lines: Vec<&str> = csv.lines().collect();
    assert_eq!(lines.len(), 39);
    assert!(lines.iter().all(|l| l.split(',').count() == 79));
    // The value at data point 3 (row 26, column 30) is the data value.
    assert_eq!(lines[25].split(',').nth(29), Some("43"));
    assert!(
        run(&deck("example-1-3.sym")).unwrap().maps[0]
            .values_csv()
            .is_none()
    );
}

#[test]
fn elective_28_dot_maps() {
    let dots = |extra: &str| {
        let d = deck("example-1-20.sym").replace(
            "    2            0.0",
            &format!("{extra}\n    2            0.0"),
        );
        let m = run(&d).unwrap().maps.remove(0);
        let count = |zone: usize| -> usize {
            // Count dots in the cells of a zone's rows around its centre.
            let t = &m.table[zone - 1];
            (t.row - 2..=t.row + 2)
                .flat_map(|r| (t.col - 3..=t.col + 3).map(move |c| (r as usize, c as usize)))
                .filter(|&(r, c)| m.at(r, c) == Strikes::new("."))
                .count()
        };
        let (low, high) = (count(2), count(3));
        (m, low, high)
    };
    let (m, low, high) = dots("   28");
    // Level 1 zone (12) gets a fifth of its cells dotted; level 5 zones all.
    assert!(low < high, "{low} {high}");
    // Zones carry dots, not tone: no solid symbols around zone 3's centre.
    let t = &m.table[2];
    let solid = (t.row - 2..=t.row + 2)
        .flat_map(|r| (t.col - 3..=t.col + 3).map(move |c| (r as usize, c as usize)))
        .filter(|&(r, c)| m.at(r, c).plain() == '█')
        .count();
    assert_eq!(solid, 0);
    let (_, low2, high2) = dots("   28       1.0");
    assert!(low2 < high2);
}
