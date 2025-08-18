use oxidized::features::syntax::SyntaxHighlighter;

#[test]
fn detect_language_by_extension_md() {
    let highlighter = SyntaxHighlighter::new().expect("init highlighter");
    let lang = highlighter
        .detect_language_from_extension("notes.md")
        .unwrap();
    assert_eq!(lang, "markdown");
}

#[test]
fn highlight_heading_line() {
    let mut highlighter = SyntaxHighlighter::new().expect("init highlighter");
    let line = "# Heading";
    let highlights = highlighter
        .highlight_text(line, "markdown")
        .expect("highlight heading");
    assert!(
        !highlights.is_empty(),
        "expected some highlights for atx heading"
    );
}

#[test]
fn highlight_inline_code_line() {
    let mut highlighter = SyntaxHighlighter::new().expect("init highlighter");
    let line = "Use `code` here";
    let highlights = highlighter
        .highlight_text(line, "markdown")
        .expect("highlight inline code");
    assert!(
        !highlights.is_empty(),
        "expected some highlights for inline code"
    );
}
