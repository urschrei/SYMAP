use symap::{Cell, MapKind, run};

fn deck(name: &str) -> String {
    std::fs::read_to_string(format!(
        "{}/examples/decks/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

#[test]
fn hills_deck_renders_contour_and_proximal() {
    let r = run(&deck("hills.sym")).unwrap();
    assert!(r.diagnostics.is_empty(), "{:?}", r.diagnostics);
    assert_eq!(r.maps.len(), 2);
    let (c, p) = (&r.maps[0], &r.maps[1]);
    assert_eq!(c.kind, MapKind::Contour);
    assert_eq!(p.kind, MapKind::Proximal);
    assert_eq!(c.classes.levels(), 6);
    // 6.0 x 10.0 inch box at 6 lines / 10 chars per inch.
    assert!(c.rows <= 36 && c.cols <= 100);
    // Every level is used, and the outline leaves corners blank.
    assert!(c.counts.iter().all(|&n| n > 0));
    assert_eq!(c.cell(0, 0), Cell::Blank);
    // 40 data points marked (some may share a cell).
    let marks = c
        .cells
        .iter()
        .filter(|x| matches!(x, Cell::Mark(_)))
        .count();
    assert!((30..=40).contains(&marks), "{marks}");
    assert!(p.cells.iter().all(|x| !matches!(x, Cell::Mark(_))));
}

#[test]
fn districts_deck_fills_zones() {
    let r = run(&deck("districts.sym")).unwrap();
    let m = &r.maps[0];
    assert_eq!(m.kind, MapKind::Conformant);
    assert!(m.cells.iter().all(|c| *c != Cell::Blank));
    // Six equal districts: frequencies are multiples of one sixth.
    let total: usize = m.counts.iter().sum();
    for &n in &m.counts {
        assert_eq!((n * 6) % total, 0);
    }
}

#[test]
fn text_and_asa_outputs_agree_in_shape() {
    let r = run(&deck("hills.sym")).unwrap();
    let m = &r.maps[0];
    let text = m.to_text();
    let asa = m.to_asa();
    let advances = asa.lines().filter(|l| !l.starts_with('+')).count();
    assert_eq!(advances, text.lines().count());
    assert!(asa.starts_with('1'));
    assert!(
        asa.lines().any(|l| l.starts_with('+')),
        "dark levels should overprint"
    );
    assert!(text.contains("FREQUENCY %"));
}

#[test]
fn value_count_mismatch_is_reported_at_the_map() {
    let d = "B-DATA POINTS\n0 0\n1 1\n99999\nE-VALUES\n5\n99999\nF-MAP\nT\n\n\n99999\n999999\n";
    let e = run(d).unwrap_err();
    assert_eq!(e.line, Some(8));
    assert!(e.message.contains("2 data points but 1 values"));
}

#[test]
fn explicit_breaks_and_unimplemented_electives() {
    let d = "B-DATA POINTS\n0 0\n0 10\n10 0\n10 10\n99999\nE-VALUES\n1\n2\n3\n4\n99999\n\
             F-MAP\nT\n\n\n   13       5.0\n   27\n*BREAKS 0 2.5 5\n*TYPE PROXIMAL\n99999\n999999\n";
    let r = run(d).unwrap();
    let m = &r.maps[0];
    assert_eq!(m.classes.breaks, vec![0.0, 2.5, 5.0]);
    assert_eq!(m.warnings.len(), 2);
    // Top-left quadrant is nearest the value-1 point: level 1.
    assert_eq!(m.cell(0, 0), Cell::Level(0));
    assert_eq!(m.cell(m.rows - 1, m.cols - 1), Cell::Level(1));
}

#[test]
fn deck_without_map_is_an_error() {
    assert!(run("B-DATA POINTS\n0 0\n99999\n999999\n").is_err());
}
