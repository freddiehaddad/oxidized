// Drift guard test: ensures the embedded keymaps snapshot (compile-time)
// matches the runtime filesystem copy. Fails fast if they diverge so tests
// relying on KeyHandler::test_with_embedded() stay in sync with user defaults.
//
// If this test fails legitimately (you edited keymaps.toml), update the
// repository file only; the embedded include_str! will reflect it automatically
// on next compile. If you *intend* to allow divergence, delete or adapt this
// test.

#[test]
fn keymaps_embedded_matches_filesystem() {
    const EMBEDDED: &str = include_str!("../keymaps.toml");
    let fs_contents =
        std::fs::read_to_string("keymaps.toml").expect("keymaps.toml should exist at repo root");

    // Normalize: unify line endings, trim trailing whitespace lines.
    fn normalize(s: &str) -> String {
        s.replace('\r', "")
            .lines()
            .map(|l| l.trim_end_matches([' ', '\t']))
            .collect::<Vec<_>>()
            .join("\n")
    }

    let embedded_norm = normalize(EMBEDDED);
    let fs_norm = normalize(&fs_contents);

    if embedded_norm != fs_norm {
        // Produce a concise diff (first 5 differing lines) to aid debugging.
        let emb_lines: Vec<_> = embedded_norm.lines().collect();
        let fs_lines: Vec<_> = fs_norm.lines().collect();
        let mut diffs = Vec::new();
        let max = emb_lines.len().max(fs_lines.len());
        for i in 0..max {
            let a = emb_lines.get(i).copied().unwrap_or("<EOF>");
            let b = fs_lines.get(i).copied().unwrap_or("<EOF>");
            if a != b {
                diffs.push(format!(
                    "Line {}:\n  embedded: {}\n  fs      : {}",
                    i + 1,
                    a,
                    b
                ));
            }
            if diffs.len() >= 5 {
                break;
            }
        }
        panic!(
            "keymaps.toml drift detected (showing up to 5 diffs):\n{}",
            diffs.join("\n\n")
        );
    }
}
