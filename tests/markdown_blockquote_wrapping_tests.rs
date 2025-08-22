use oxidized::utils::markdown::render_markdown;

fn lines(input: &str) -> Vec<String> {
    input.split('\n').map(|s| s.to_string()).collect()
}

#[test]
fn blockquote_soft_breaks_collapse_into_single_line() {
    // New behavior: consecutive blockquote source lines (soft breaks) collapse into a single
    // preview line separated by spaces, sharing one prefix. (Wrapping to multiple visual
    // rows happens later in the UI layer based on width.)
    let md = "> first line\n> second line\n> third line";
    let render = render_markdown(&lines(md), "off", "none");
    assert!(
        !render.lines.is_empty(),
        "rendered lines: {:?}",
        render.lines
    );
    let first = &render.lines[0];
    assert!(
        first.starts_with("▎ "),
        "missing blockquote prefix: {:?}",
        first
    );
    assert!(first.contains("first line"));
    assert!(first.contains("second line"));
    assert!(first.contains("third line"));
    // Ensure no additional non-empty quoted lines (they should be merged)
    for extra in render.lines.iter().skip(1) {
        if !extra.is_empty() {
            panic!("unexpected additional quoted line: {:?}", render.lines);
        }
    }
}

#[test]
fn blockquote_then_paragraph_has_single_blank_line_after_collapsed_quote() {
    let md = "> line one\n> line two\n\nAfter";
    let render = render_markdown(&lines(md), "off", "none");

    assert!(
        render.lines.len() >= 3,
        "rendered lines: {:?}",
        render.lines
    );
    assert!(render.lines[0].starts_with("▎ "));
    assert!(render.lines[0].contains("line one"));
    assert!(render.lines[0].contains("line two"));
    assert_eq!(
        render.lines[1], "",
        "expected blank separator line: {:?}",
        render.lines
    );
    assert_eq!(render.lines[2], "After");
}
