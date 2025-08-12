use oxidized::features::syntax::{
    HighlightCacheEntry, HighlightCacheKey, HighlightRange, HighlightStyle,
};

#[test]
fn cache_key_same_inputs_produce_same_key() {
    let content = "fn main() {}";
    let lang = "rust";

    let k1 = HighlightCacheKey::new_simple(content, lang);
    let k2 = HighlightCacheKey::new_simple(content, lang);

    // Same content + language + current theme -> same key
    assert_eq!(k1, k2);
}

#[test]
fn cache_key_changes_with_content() {
    let lang = "rust";
    let k1 = HighlightCacheKey::new_simple("let a = 1;", lang);
    let k2 = HighlightCacheKey::new_simple("let b = 2;", lang);
    assert_ne!(k1, k2, "different content should yield different keys");
}

#[test]
fn cache_key_changes_with_language() {
    let content = "fn f() {}";
    let k_rust = HighlightCacheKey::new_simple(content, "rust");
    let k_other = HighlightCacheKey::new_simple(content, "other-lang");
    assert_ne!(
        k_rust, k_other,
        "different language should yield different keys"
    );
}

#[test]
fn cache_key_is_stable_given_current_theme() {
    // We cannot change the theme from here; ensure stability across calls
    let content = "fn f() {}";
    let lang = "rust";
    let a = HighlightCacheKey::new_simple(content, lang);
    let b = HighlightCacheKey::new_simple(content, lang);
    assert_eq!(a, b);
}

#[test]
fn cache_entry_wraps_highlights() {
    let highlights = vec![
        HighlightRange {
            start: 0,
            end: 2,
            style: HighlightStyle {
                fg_color: Some("#ffffff".to_string()),
                bg_color: None,
                bold: false,
                italic: false,
                underline: false,
            },
        },
        HighlightRange {
            start: 3,
            end: 7,
            style: HighlightStyle {
                fg_color: Some("#000000".to_string()),
                bg_color: None,
                bold: true,
                italic: false,
                underline: false,
            },
        },
    ];

    let entry = HighlightCacheEntry::new(highlights.clone());
    let got = entry.highlights();
    assert_eq!(got.len(), 2);
    assert_eq!(got[0].start, highlights[0].start);
    assert_eq!(got[1].end, highlights[1].end);
}
