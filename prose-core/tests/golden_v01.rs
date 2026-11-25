//! Guards the v0.1 reading: `render` output for every fixture must stay
//! byte-identical across releases. Regenerate a golden only for a
//! deliberate, announced change to v0.1 behaviour.
use prose_core::render;

#[test]
fn render_output_matches_goldens() {
    let dir = format!("{}/tests", env!("CARGO_MANIFEST_DIR"));
    let mut seen = 0;
    let mut names: Vec<_> = std::fs::read_dir(format!("{dir}/fixtures"))
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .filter(|n| n.ends_with(".json"))
        .collect();
    names.sort();
    for name in names {
        let text = std::fs::read_to_string(format!("{dir}/fixtures/{name}")).unwrap();
        let stem = name.trim_end_matches(".json");
        let got = format!("{:#?}\n", render(&text));
        let path = format!("{dir}/golden/{stem}.txt");
        if std::env::var_os("PROSE_BLESS_GOLDEN").is_some() && !std::path::Path::new(&path).exists()
        {
            std::fs::write(&path, &got).unwrap();
        }
        let want =
            std::fs::read_to_string(&path).unwrap_or_else(|_| panic!("missing golden {path}"));
        assert_eq!(got, want, "golden mismatch for {name}");
        seen += 1;
    }
    assert!(seen >= 13, "expected all fixtures, saw {seen}");
}
