use oxidized::utils::markdown::render_markdown;

fn lines(input: &str) -> Vec<String> {
    input.split('\n').map(|s| s.to_string()).collect()
}

#[test]
fn line_map_handles_headings_and_underlines() {
    let src = "# Title\nAfter heading\n\n## Sub\nBody";
    let render = render_markdown(&lines(src), "off", "none");
    // Preview lines should be: ["Title", "=====", "", "After heading", "", "Sub", "-----", "", "Body"]
    assert!(render.lines[0] == "Title");
    assert!(render.lines[1].chars().all(|c| c == '='));
    assert_eq!(render.lines[2], "");
    // Mapping expectations:
    // 0 ("# Title") -> preview 0 (the title line)
    // 1 ("After heading") -> preview 3 (after title+underline+blank)
    // 2 (blank) -> preview 4 (blank between para and next heading)
    // 3 ("## Sub") -> preview 5 (the sub title)
    // 4 ("Body") -> preview 8 (after sub title+underline+blank)
    let m = &render.src_to_preview;
    assert!(m.len() >= 5, "map too short: {:?}", m);
    assert_eq!(m[0], 0);
    assert_eq!(m[1], 3);
    assert!(m[2] == 4 || m[2] == 3, "blank maps near 4: {}", m[2]);
    assert!(
        m[3] == 5 || m[3] == 6,
        "sub heading maps to its title line: {}",
        m[3]
    );
    assert!(m[4] >= m[3], "monotonic after heading");
}

#[test]
fn line_map_handles_code_fences_and_code_lines() {
    let src = "```rust\nfn main(){}\n```\nafter";
    let render = render_markdown(&lines(src), "off", "none");
    // Preview should be: ["    fn main(){}", "", "after"]
    assert_eq!(render.lines[0], "    fn main(){}");
    assert_eq!(render.lines[1], "");
    assert_eq!(render.lines[2], "after");

    // Mapping: open fence -> 0, code line -> 0, closing fence -> 1, after -> 2
    let m = &render.src_to_preview;
    assert_eq!(m.len(), 4);
    assert_eq!(m[0], 0);
    assert_eq!(m[1], 0);
    assert!(m[2] == 1 || m[2] == 0); // allow slight variance if blank collapsing changes
    assert_eq!(m[3], 2);
    // Monotonic guarantee
    assert!(m.windows(2).all(|w| w[0] <= w[1]));
}

#[test]
fn line_map_handles_inline_html_ignored_without_math() {
    let src = "<span>html</span>\ntext";
    let render = render_markdown(&lines(src), "off", "none");
    // Pulldown-cmark may emit inner text for inline HTML; do not assert exact content.
    let m = &render.src_to_preview;
    // The HTML line should not force an advance such that the next line can't map to the top.
    assert_eq!(m[0], 0);
    assert_eq!(m[1], 0);
}

#[test]
fn line_map_handles_lists_and_blockquotes() {
    let src = "> quote\n- item 1\n- item 2";
    let render = render_markdown(&lines(src), "off", "none");
    assert!(render.lines[0].starts_with("▎ "));
    // Renderer inserts a blank line between blockquote and subsequent list
    assert_eq!(render.lines[1], "");
    assert_eq!(render.lines[2], "• item 1");
    assert_eq!(render.lines[3], "• item 2");
    let m = &render.src_to_preview;
    assert_eq!(m[0], 0);
    assert!(m[1] >= 2);
    assert!(m[2] >= 3);
}

#[test]
fn line_map_is_monotonic_even_with_odd_content() {
    let src = "# A\n```\ncode\n```\n<div>drop</div>\npara";
    let render = render_markdown(&lines(src), "off", "none");
    assert!(render.src_to_preview.windows(2).all(|w| w[0] <= w[1]));
}

#[test]
fn line_map_for_softbreak_paragraph_lines() {
    // Two source lines that render into one preview line (soft breaks become spaces)
    let src = "hello\nworld";
    let render = render_markdown(&lines(src), "off", "none");
    assert!(!render.lines.is_empty());
    let m = &render.src_to_preview;
    assert_eq!(m[0], 0);
    assert!(
        m[1] == 0 || m[1] == 1,
        "second line maps to same or immediate next due to trailing blank: {}",
        m[1]
    );
}

#[test]
fn line_map_for_links_without_urls() {
    // Source contains markdown link syntax; preview renders bracketed text only
    let src = "See [Link](http://example) here";
    let render = render_markdown(&lines(src), "off", "none");
    assert!(!render.lines.is_empty());
    // The mapping should still point to preview line 0
    assert_eq!(render.src_to_preview.first().copied().unwrap_or(0), 0);
}

#[test]
fn line_map_for_nested_lists_and_blockquotes() {
    // Nested lists inside and outside blockquotes
    let src = r#">
> - item A
>   - nested A1
>     1. ordered one
>     2. ordered two
> - item B

- top 1
  - sub 1
text
"#;
    let src_lines = lines(src);
    let render = render_markdown(&src_lines, "off", "none");

    // Preview should contain bullets and ordered items (quote prefix may or may not appear on list items)
    let find = |s: &str| render.lines.iter().position(|l| l.contains(s)).expect(s);
    let p_item_a = find("item A");
    let p_nested_a1 = find("nested A1");
    let p_ord1 = find("1. ordered one");
    let p_ord2 = find("2. ordered two");
    let p_item_b = find("item B");
    let p_top1 = find("top 1");
    let p_sub1 = find("sub 1");
    let p_text = find("text");

    // Ensure quoted items appear before top-level list and paragraph without requiring exact adjacency
    let quoted_max = *[p_item_a, p_nested_a1, p_ord1, p_ord2, p_item_b]
        .iter()
        .max()
        .unwrap();
    let toplevel_min = *[p_top1, p_sub1, p_text].iter().min().unwrap();
    assert!(quoted_max < toplevel_min);

    let m = &render.src_to_preview;
    // Source indices for the above (0-based):
    // 0:'>' 1:'> - item A' 2:'>   - nested A1' 3:'>     1. ordered one' 4:'>     2. ordered two'
    // 5:'> - item B' 6:'' 7:'- top 1' 8:'  - sub 1' 9:'text'
    // Allow an off-by-one before each item due to optional blank lines around quoted lists
    assert!(m[1] == p_item_a || m[1] + 1 == p_item_a);
    assert!(m[2] == p_nested_a1 || m[2] + 1 == p_nested_a1);
    assert!(m[3] == p_ord1 || m[3] + 1 == p_ord1);
    assert!(m[4] == p_ord2 || m[4] + 1 == p_ord2);
    assert!(m[5] == p_item_b || m[5] + 1 == p_item_b);
    assert!(m[7] == p_top1 || m[7] + 1 == p_top1);
    assert!(m[8] == p_sub1 || m[8] + 1 == p_sub1);
    // Allow a small proximity window (±2) since spacing can shift with preceding blocks
    let delta = m[9].abs_diff(p_text);
    assert!(
        delta <= 2,
        "text mapping too far: src->{} vs preview at {} (Δ={})",
        m[9],
        p_text,
        delta
    );
    // Monotonic mapping overall
    assert!(m.windows(2).all(|w| w[0] <= w[1]));
}

#[test]
fn line_map_for_multiline_html_blocks_without_math() {
    // A multi-line HTML block should be ignored when math_mode is off
    let src = "<div>\n<p>Hi</p>\n<span>Inline</span>\n</div>\nAfter";
    let src_lines = lines(src);
    let render = render_markdown(&src_lines, "off", "none");
    let m = &render.src_to_preview;
    // First four source lines are HTML; they should not force advancing the preview index
    assert_eq!(m[0], m[1]);
    assert_eq!(m[1], m[2]);
    assert_eq!(m[2], m[3]);
    // With no intervening blank line, CommonMark treats the following text as part of the HTML block.
    // Our renderer ignores HTML, so the mapping for "After" stays at the same preview index.
    assert_eq!(m[4], m[0]);
    assert!(m.windows(2).all(|w| w[0] <= w[1]));
}

#[test]
fn line_map_for_mixed_constructs() {
    let src = r#"# Heading
> Quote
- list
```
code
```
<div>ignored</div>

para
"#;
    let src_lines = lines(src);
    let render = render_markdown(&src_lines, "off", "none");
    // Locate key preview markers
    let find = |s: &str| render.lines.iter().position(|l| l.contains(s)).expect(s);
    let p_heading = find("Heading");
    // Underline should be right after heading
    assert!(
        render
            .lines
            .get(p_heading + 1)
            .map(|l| l.chars().all(|c| c == '-' || c == '='))
            .unwrap_or(false)
    );
    let p_list = find("list");
    let p_code = find("code");
    let p_para = find("para");

    // Mapping checks: heading source (0) maps to heading preview; list and code map to their lines; para comes after code
    let m = &render.src_to_preview;
    assert_eq!(m[0], p_heading);
    assert!(m[2] == p_list);
    assert!(m[4] == p_code); // 'code' is the 4th source line (0-based): 3 is ``` open, 4 is code
    assert!(m[8] == p_para);
    // Monotonic and clamped
    assert!(m.windows(2).all(|w| w[0] <= w[1]));
}
