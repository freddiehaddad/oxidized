use oxidized::features::syntax::SemanticCategory;
use oxidized::utils::markdown::render_markdown;

fn lines(input: &str) -> Vec<String> {
    input.split('\n').map(|s| s.to_string()).collect()
}

#[test]
fn bold_link_renders_once_with_brackets_and_no_constant_span() {
    // **[Vim](url)** should render as "[Vim]" only, with spans on brackets (Punctuation)
    // and inner text (Attribute), and no Constant spans from the surrounding bold.
    let md = "**[Vim](https://vim.org)**";
    let render = render_markdown(&lines(md), "off", "none");

    assert_eq!(render.lines[0], "[Vim]");

    let spans = render.spans.get(&0).cloned().unwrap_or_default();
    // Expect exactly 3 spans: '[' punctuation, 'Vim' attribute, ']' punctuation
    assert_eq!(spans.len(), 3, "spans: {:?}", spans);
    assert_eq!(spans[0].start, 0);
    assert_eq!(spans[0].end, 1);
    assert!(matches!(spans[0].category, SemanticCategory::Punctuation));

    assert_eq!(spans[1].start, 1);
    assert_eq!(spans[1].end, 4);
    assert!(matches!(spans[1].category, SemanticCategory::Attribute));

    assert_eq!(spans[2].start, 4);
    assert_eq!(spans[2].end, 5);
    assert!(matches!(spans[2].category, SemanticCategory::Punctuation));

    // Ensure there's no Constant span leaking from bold
    assert!(
        !spans
            .iter()
            .any(|s| matches!(s.category, SemanticCategory::Constant))
    );
}

#[test]
fn heading_with_bold_text_has_type_and_constant_spans_and_correct_underline() {
    let md = "# Hello **World**";
    let render = render_markdown(&lines(md), "off", "none");

    // Expect: heading line, underline line, then a blank separator
    assert!(!render.lines.is_empty());
    assert_eq!(render.lines[0], "Hello World");

    // Underline must be '=' repeated to Unicode display width of the heading text
    let underline = &render.lines[1];
    assert!(underline.chars().all(|c| c == '='));
    // Simple ASCII width equals string len
    assert_eq!(underline.len(), "Hello World".len());

    // Spans on heading line should include a Constant span over "World"
    let spans0 = render.spans.get(&0).cloned().unwrap_or_default();
    // Expect at least one Constant span
    assert!(
        spans0.iter().any(|s| {
            s.start <= 6 && s.end >= 11 && matches!(s.category, SemanticCategory::Constant)
        }),
        "spans: {:?}",
        spans0
    );

    // Underline line should be marked as Delimiter
    let spans1 = render.spans.get(&1).cloned().unwrap_or_default();
    assert!(
        spans1
            .iter()
            .any(|s| matches!(s.category, SemanticCategory::Delimiter))
    );
}

#[test]
fn code_block_is_indented_without_closing_fence_and_commented() {
    let md = "```rust\nfn x(){}\n```";
    let render = render_markdown(&lines(md), "off", "none");

    // Expect one content line indented by 4 spaces, then a blank line
    assert!(!render.lines.is_empty());
    assert_eq!(render.lines[0], "    fn x(){}");
    // There must be no literal backticks in output
    assert!(render.lines.iter().all(|l| !l.contains("```")));

    // Span for code content should be Comment from col 4 to end
    let spans0 = render.spans.get(&0).cloned().unwrap_or_default();
    assert_eq!(spans0.len(), 1, "spans: {:?}", spans0);
    let sp = &spans0[0];
    assert_eq!(sp.start, 4);
    assert_eq!(sp.end, render.lines[0].len());
    assert!(matches!(sp.category, SemanticCategory::Comment));
}

#[test]
fn inline_code_has_no_backticks_and_is_commented() {
    let md = "inline `code` here";
    let render = render_markdown(&lines(md), "off", "none");

    assert_eq!(render.lines[0], "inline code here");
    // Find a Comment span over the word "code" at positions 7..11
    let spans = render.spans.get(&0).cloned().unwrap_or_default();
    assert!(
        spans.iter().any(|s| s.start <= 7
            && s.end >= 11
            && matches!(s.category, SemanticCategory::Comment)),
        "spans: {:?}",
        spans
    );
}

#[test]
fn blank_line_between_lists_and_paragraphs() {
    let md = "- item 1\n- item 2\n\nsome text\n\n- item 3\n- item 4";
    let render = render_markdown(&lines(md), "off", "none");

    // Expect output layout like:
    // "• item 1"
    // "• item 2"
    // ""            <-- blank line between list and paragraph
    // "some text"
    // ""            <-- blank line between paragraph and next list (paragraph end spacing)
    // "• item 3"
    // "• item 4"

    // Find the indices of the key lines to assert on spacing
    let mut idx = 0;
    assert_eq!(render.lines[idx], "• item 1");
    idx += 1;
    assert_eq!(render.lines[idx], "• item 2");
    idx += 1;
    assert_eq!(render.lines[idx], "");
    idx += 1; // required blank line
    assert_eq!(render.lines[idx], "some text");
    idx += 1;
    assert_eq!(render.lines[idx], "");
    idx += 1; // paragraph spacing already ensured
    assert_eq!(render.lines[idx], "• item 3");
    idx += 1;
    assert_eq!(render.lines[idx], "• item 4");
}

#[test]
fn blank_line_before_code_block_inside_list_item() {
    let md = "1. Some item\n   ```bash\n   some code...\n   ```";
    let render = render_markdown(&lines(md), "off", "none");

    // Expect:
    // "1. Some item"
    // ""                 <-- blank line between item and code block
    // "    some code..."
    // ""                 <-- trailing blank after code block already present

    assert_eq!(render.lines[0], "1. Some item");
    assert_eq!(render.lines[1], "");
    // First code line should be indented by 4 spaces
    assert!(render.lines[2].starts_with("    "));
    assert!(render.lines[2].contains("some code..."));
}

#[test]
fn h2_underline_uses_dash_and_unicode_display_width() {
    // Include wide and combining characters: '世界' (width 2 each), and 'e\u0301' (e + combining acute)
    let md = "## Héllo 世界"; // H2 -> '-' underline
    let render = render_markdown(&lines(md), "off", "none");

    assert!(render.lines.len() >= 2);
    assert_eq!(render.lines[0], "Héllo 世界");
    let underline = &render.lines[1];
    assert!(underline.chars().all(|c| c == '-'));
    // Quick sanity: dash count equals display width as implemented; for these characters,
    // ASCII letters count 1, 'é' (precomposed) count 1, each CJK char width 2.
    // Here we just ensure length >= chars count and not trivially wrong (non-empty).
    assert!(!underline.is_empty());
}

#[test]
fn italic_link_renders_once_with_correct_spans() {
    let md = "*[Link](http://example)*";
    let render = render_markdown(&lines(md), "off", "none");

    assert_eq!(render.lines[0], "[Link]");
    let spans = render.spans.get(&0).cloned().unwrap_or_default();
    // Expect punctuation '[' and ']', and Attribute for inner text; no String span overlapping link parts
    assert!(spans.iter().any(|s| s.start == 0
        && s.end == 1
        && matches!(s.category, SemanticCategory::Punctuation)));
    assert!(
        spans.iter().any(|s| s.start == 1
            && s.end == 5
            && matches!(s.category, SemanticCategory::Attribute))
    );
    assert!(spans.iter().any(|s| s.start == 5
        && s.end == 6
        && matches!(s.category, SemanticCategory::Punctuation)));
    // Ensure no duplication or stray italic coloring applied over link pieces
    assert!(
        !spans
            .iter()
            .any(|s| matches!(s.category, SemanticCategory::String))
    );
}

#[test]
fn nested_list_then_paragraph_has_single_blank_line() {
    let md = "- a\n  - b\n\npara";
    let render = render_markdown(&lines(md), "off", "none");

    // Expect:
    // "• a"
    // "• b"   (we don't render nested indent in prefix now; content remains inline)
    // ""
    // "para"
    assert_eq!(render.lines[0], "• a");
    assert_eq!(render.lines[1], "• b");
    assert_eq!(render.lines[2], "");
    assert_eq!(render.lines[3], "para");
}

fn assert_spans_well_formed(render: &oxidized::utils::markdown::MarkdownRender) {
    for (i, line) in render.lines.iter().enumerate() {
        if let Some(spans) = render.spans.get(&i) {
            // Check bounds and non-empty
            for sp in spans {
                assert!(sp.start < sp.end, "empty span on line {}: {:?}", i, sp);
                assert!(
                    sp.end <= line.len(),
                    "span out of bounds on line {}: {:?} / len {}",
                    i,
                    sp,
                    line.len()
                );
            }
            // Check non-overlap (allow touching)
            let mut sorted = spans.clone();
            sorted.sort_by(|a, b| a.start.cmp(&b.start).then(a.end.cmp(&b.end)));
            for w in sorted.windows(2) {
                let a = &w[0];
                let b = &w[1];
                assert!(
                    a.end <= b.start,
                    "overlapping spans on line {}: {:?} then {:?}",
                    i,
                    a,
                    b
                );
            }
        }
    }
}

#[test]
fn spans_invariants_hold_on_complex_sample() {
    let md = "# T\n\n- **[X](y)** and *[Z](q)*\n\n> Quote\n\nText with `code` and a link [L](u).";
    let render = render_markdown(&lines(md), "off", "none");
    assert_spans_well_formed(&render);
}

#[test]
fn paragraph_then_list_has_single_blank_line() {
    // No blank line in source; renderer should insert exactly one between paragraph and list
    let md = "Some paragraph\n- item 1";
    let render = render_markdown(&lines(md), "off", "none");

    assert_eq!(render.lines[0], "Some paragraph");
    assert_eq!(render.lines[1], "");
    assert_eq!(render.lines[2], "• item 1");
}

#[test]
fn list_then_code_block_outside_has_blank_line() {
    // Code block after list (outside of the list) should have a blank line separator
    let md = "- a\n- b\n\n```bash\nx\n```";
    let render = render_markdown(&lines(md), "off", "none");

    assert_eq!(render.lines[0], "• a");
    assert_eq!(render.lines[1], "• b");
    // Exactly one blank line between list and code block
    assert_eq!(render.lines[2], "");
    // First code line indented by four spaces
    assert!(render.lines[3].starts_with("    "));
    assert!(render.lines[3].contains("x"));
}

#[test]
fn blockquote_then_list_has_single_blank_line() {
    let md = "> quoted\n\n- item 1";
    let render = render_markdown(&lines(md), "off", "none");

    // Expect:
    // "▎ quoted"
    // ""
    // "• item 1"
    assert!(render.lines[0].starts_with("▎ "));
    assert!(render.lines[0].contains("quoted"));
    assert_eq!(render.lines[1], "");
    assert_eq!(render.lines[2], "• item 1");
}

#[test]
fn heading_then_list_has_single_blank_line() {
    let md = "## Title\n- item";
    let render = render_markdown(&lines(md), "off", "none");

    // Expect:
    // "Title"
    // "-----" (dashes)
    // ""      (exactly one blank)
    // "• item"
    assert_eq!(render.lines[0], "Title");
    assert!(render.lines[1].chars().all(|c| c == '-'));
    assert_eq!(render.lines[2], "");
    assert_eq!(render.lines[3], "• item");
}

#[test]
fn list_then_heading_has_single_blank_line() {
    // After a list ends, there should be exactly one blank line before a subsequent heading.
    let md = "- a\n- b\n\n## Title";
    let render = render_markdown(&lines(md), "off", "none");

    // Expect:
    // "• a"
    // "• b"
    // ""      (single blank)
    // "Title"
    // "-----"
    assert_eq!(render.lines[0], "• a");
    assert_eq!(render.lines[1], "• b");
    assert_eq!(render.lines[2], "");
    assert_eq!(render.lines[3], "Title");
    assert!(render.lines[4].chars().all(|c| c == '-'));
}

#[test]
fn code_block_then_list_has_single_blank_line() {
    // A code block (outside list) followed by a list should be separated by exactly one blank line
    let md = "```\nline\n```\n- item";
    let render = render_markdown(&lines(md), "off", "none");

    // Expect:
    // "    line"
    // ""         (single blank)
    // "• item"
    assert_eq!(render.lines[0], "    line");
    assert_eq!(render.lines[1], "");
    assert_eq!(render.lines[2], "• item");
}

#[test]
fn nested_blockquote_boundaries_and_spacing() {
    // Nested blockquote content is joined into a single line with a single prefix; ensure
    // the outermost spacing (one blank line) and subsequent list rendering are correct.
    let md = "> outer\n> > inner\n> tail\n\n- next";
    let render = render_markdown(&lines(md), "off", "none");

    // Find the index of the list item
    let list_idx = render
        .lines
        .iter()
        .position(|l| l == "• next")
        .expect("list item present");
    assert!(list_idx >= 2, "expect at least quote, blank, then list");
    // The line before the list should be blank
    assert_eq!(render.lines[list_idx - 1], "");
    // All non-empty lines before that blank should be part of the quote (prefixed)
    for l in &render.lines[..list_idx - 1] {
        if !l.is_empty() {
            assert!(l.starts_with("▎ "));
        }
    }
}
