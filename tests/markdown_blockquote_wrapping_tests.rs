use oxidized::utils::markdown::render_markdown;

fn lines(input: &str) -> Vec<String> {
    input.split('\n').map(|s| s.to_string()).collect()
}

#[test]
fn blockquote_soft_breaks_preserve_prefix_on_each_line() {
    // In a blockquote, consecutive quoted lines are soft breaks within a paragraph.
    // Each source line should render on its own line and keep the quote prefix.
    let md = "> first line\n> second line\n> third line";
    let render = render_markdown(&lines(md), "off", "none");

    assert!(
        render.lines.len() >= 3,
        "rendered lines: {:?}",
        render.lines
    );
    assert!(render.lines[0].starts_with("▎ "));
    assert!(render.lines[1].starts_with("▎ "));
    assert!(render.lines[2].starts_with("▎ "));
    assert!(render.lines[0].contains("first line"));
    assert!(render.lines[1].contains("second line"));
    assert!(render.lines[2].contains("third line"));
}

#[test]
fn blockquote_then_paragraph_has_single_blank_line_and_prefix_is_preserved() {
    let md = "> line one\n> line two\n\nAfter";
    let render = render_markdown(&lines(md), "off", "none");

    // Expect the two quoted lines, then exactly one blank line, then the paragraph
    assert!(
        render.lines.len() >= 4,
        "rendered lines: {:?}",
        render.lines
    );
    assert!(render.lines[0].starts_with("▎ "));
    assert!(render.lines[1].starts_with("▎ "));
    assert_eq!(render.lines[2], "");
    assert_eq!(render.lines[3], "After");
}
