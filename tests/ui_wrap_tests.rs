use oxidized::ui::UI;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

#[test]
fn wrap_ascii_exact_fit_and_overflow() {
    let ui = UI::new();
    let s = "abcdef";
    // width 3 should take first 3 chars
    let (end, count) = ui.wrap_next_end_byte(s, 0, 3, false);
    assert_eq!(&s[0..end], "abc");
    assert_eq!(count, 3);
    // next segment from end
    let (end2, count2) = ui.wrap_next_end_byte(s, end, 3, false);
    assert_eq!(&s[end..end2], "def");
    assert_eq!(count2, 3);
}

#[test]
fn wrap_emoji_width_two_no_split() {
    let ui = UI::new();
    let s = "A🙂B"; // 🙂 is width 2
    // width 3 should fit 'A' (1) + '🙂' (2) exactly
    let (end, count) = ui.wrap_next_end_byte(s, 0, 3, false);
    assert_eq!(&s[0..end], "A🙂");
    assert_eq!(count, 2); // 2 graphemes
    // remaining is 'B'
    let (end2, count2) = ui.wrap_next_end_byte(s, end, 3, false);
    assert_eq!(&s[end..end2], "B");
    assert_eq!(count2, 1);
}

#[test]
fn wrap_combining_graphemes_width_one_each() {
    let ui = UI::new();
    let s = "e\u{0301}e\u{0301}"; // two é graphemes
    // width 2 should include both graphemes
    let (end, count) = ui.wrap_next_end_byte(s, 0, 2, false);
    assert_eq!(count, 2);
    let slice = &s[0..end];
    // Should equal the first two graphemes
    assert_eq!(slice, "e\u{0301}e\u{0301}");
    // From end, there should be nothing left
    let (end2, count2) = ui.wrap_next_end_byte(s, end, 2, false);
    assert_eq!(end2, end);
    assert_eq!(count2, 0);
}

#[test]
fn word_break_prefers_space_boundary() {
    let ui = UI::new();
    let s = "word word";
    // width 5: should break after the space when word_break=true
    let (end, _count) = ui.wrap_next_end_byte(s, 0, 5, true);
    assert_eq!(&s[0..end], "word ");
    // When word_break=false, it should just take 5 columns, i.e., "word " as well
    let (end2, _count2) = ui.wrap_next_end_byte(s, 0, 5, false);
    assert_eq!(&s[0..end2], "word ");
}

#[test]
fn char_pos_to_byte_for_emoji_boundary() {
    // floor_char_boundary converts a character position (count) to byte index
    // 0 -> 0, 1 -> after first grapheme
    let s = "🙂X"; // 🙂 is 4 bytes, width 2, but one grapheme
    assert_eq!(UI::floor_char_boundary(s, 0), 0);
    assert_eq!(UI::floor_char_boundary(s, 1), "🙂".len());
    assert_eq!(UI::floor_char_boundary(s, 2), s.len());
}

#[test]
fn wrap_ascii_no_word_break_exact_and_overflow() {
    let ui = UI::new();
    let s = "hello world";
    // width 5: cut at 5 columns ("hello")
    let (end, count) = ui.wrap_next_end_byte(s, 0, 5, false);
    assert_eq!(&s[..end], "hello");
    assert_eq!(count, 5);
    // width 6: include space
    let (end2, count2) = ui.wrap_next_end_byte(s, 0, 6, false);
    assert_eq!(&s[..end2], "hello ");
    assert_eq!(count2, 6);
}

#[test]
fn wrap_ascii_word_break_prefers_space() {
    let ui = UI::new();
    let s = "word word";
    // width 7: without word break would cut at 7; with word break include up to last space
    let (end, count) = ui.wrap_next_end_byte(s, 0, 7, true);
    assert_eq!(&s[..end], "word ");
    assert_eq!(count, "word ".graphemes(true).count());
}

#[test]
fn wrap_emoji_width_two() {
    let ui = UI::new();
    let s = "A🙂B"; // 🙂 is width 2
    // width 2: only "A"
    let (end1, c1) = ui.wrap_next_end_byte(s, 0, 2, false);
    assert_eq!(&s[..end1], "A");
    assert_eq!(c1, 1);
    // width 3: "A🙂"
    let (end2, c2) = ui.wrap_next_end_byte(s, 0, 3, false);
    assert_eq!(&s[..end2], "A🙂");
    assert_eq!(c2, 2);
}

#[test]
fn wrap_combining_mark_treated_as_one_grapheme() {
    let ui = UI::new();
    let s = "e\u{0301}e\u{0301}"; // e + COMBINING ACUTE, twice
    // width 1 should take first grapheme only
    let (end, count) = ui.wrap_next_end_byte(s, 0, 1, false);
    assert_eq!(count, 1);
    let first = &s[..end];
    assert_eq!(UnicodeWidthStr::width(first), 1);
    assert_eq!(first.graphemes(true).count(), 1);
}

#[test]
fn wrap_zwj_cluster_width_two() {
    let ui = UI::new();
    let s = "👩‍💻X"; // ZWJ cluster, typically width 2
    let (end, count) = ui.wrap_next_end_byte(s, 0, 2, false);
    let seg = &s[..end];
    assert_eq!(count, 1);
    assert!(seg.contains("👩")); // segment is the emoji cluster
    assert_eq!(UnicodeWidthStr::width(seg), 2);
}

#[test]
fn floor_char_boundary_returns_valid_boundary() {
    let s = "- [🏗️ Architecture]"; // includes variation selector
    // choose a mid-range char position; must return a valid boundary
    let idx = UI::floor_char_boundary(s, 7);
    assert!(s.is_char_boundary(idx));
    // slicing at this boundary must not panic
    let _slice = &s[idx..];
}
