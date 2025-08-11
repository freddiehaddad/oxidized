use oxidized::features::search::SearchEngine;

#[test]
fn replace_ascii_case_insensitive() {
    let mut eng = SearchEngine::new();
    eng.set_case_sensitive(false);
    eng.set_use_regex(false);
    let mut text = vec!["Hello World".to_string(), "hello world".to_string()];
    let n = eng.replace("hello", "hi", &mut text);
    assert_eq!(n, 2);
    assert_eq!(text[0], "hi World");
    assert_eq!(text[1], "hi world");
}

#[test]
fn replace_ascii_case_sensitive() {
    let mut eng = SearchEngine::new();
    eng.set_case_sensitive(true);
    eng.set_use_regex(false);
    let mut text = vec!["Hello hello".to_string()];
    let n = eng.replace("Hello", "Hi", &mut text);
    assert_eq!(n, 1);
    assert_eq!(text[0], "Hi hello");
}

#[test]
fn replace_regex_case_insensitive() {
    let mut eng = SearchEngine::new();
    eng.set_use_regex(true);
    eng.set_case_sensitive(false);
    let mut text = vec!["foo1 bar FOO2".to_string(), "none".to_string()];
    let n = eng.replace(r"\bfoo\d+", "X", &mut text);
    assert_eq!(n, 2);
    assert_eq!(text[0], "X bar X");
    assert_eq!(text[1], "none");
}

#[test]
fn replace_unicode_case_insensitive() {
    let mut eng = SearchEngine::new();
    eng.set_use_regex(false);
    eng.set_case_sensitive(false);
    let mut text = vec!["Café CAFÉ".to_string()];
    let n = eng.replace("é", "E", &mut text);
    // In Unicode case-insensitive mode, both variants should be replaced
    assert_eq!(n, 2);
    assert_eq!(text[0], "CafE CAFE");
}

#[test]
fn replace_overlapping_like() {
    let mut eng = SearchEngine::new();
    eng.set_use_regex(false);
    eng.set_case_sensitive(true);
    let mut text = vec!["aaa".to_string()];
    let n = eng.replace("aa", "b", &mut text);
    // Non-overlapping advances by pattern length: "aaa" -> match at 0..2 -> "b" + "a" -> "ba"
    assert_eq!(n, 1);
    assert_eq!(text[0], "ba");
}
