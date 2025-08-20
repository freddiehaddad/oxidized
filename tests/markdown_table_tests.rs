use oxidized::utils::markdown::render_markdown;

fn lines(input: &str) -> Vec<String> {
    input.split('\n').map(|s| s.to_string()).collect()
}

#[test]
fn simple_table_renders_as_aligned_ascii() {
    let md = "| Column 1 | Column 2 | Column 3 |\n|----------|:--------:|---------:|\n| Row 1    | Data A   | Data B   |\n| Row 2    | Data C   | Data D   |\n| Row 3    | Data E   | Data F   |";
    let render = render_markdown(&lines(md), "off", "none");

    assert!(render.lines.len() >= 6, "lines: {:?}", render.lines);
    // Header, separator, 3 rows, plus a trailing blank line makes at least 6 lines
    assert!(render.lines[0].starts_with("| "));
    assert!(render.lines[1].contains("-"), "separator missing");
    assert!(render.lines[2].starts_with("| "));
    assert!(render.lines[3].starts_with("| "));
    assert!(render.lines[4].starts_with("| "));
}

#[test]
fn table_inside_blockquote_keeps_prefix() {
    let md = ">
> | A | B |\n> |---|---|\n> | x | y |";
    let render = render_markdown(&lines(md), "off", "none");
    // Expect quoted lines with prefix
    let mut saw_header = false;
    for l in &render.lines {
        if l.is_empty() {
            continue;
        }
        assert!(l.starts_with("▎ "));
        if l.contains('|') {
            saw_header = true;
        }
    }
    assert!(saw_header, "no table lines detected: {:?}", render.lines);
}
