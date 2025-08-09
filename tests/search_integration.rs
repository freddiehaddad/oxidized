use oxidized::features::search::{SearchEngine, SearchResult};

#[test]
fn test_search_engine_creation() {
    let _engine = SearchEngine::new();
    // Creation should not panic
}

#[test]
fn test_case_sensitive_search() {
    let mut engine = SearchEngine::new();
    engine.set_case_sensitive(true);

    let text = vec![
        "Hello World".to_string(),
        "hello world".to_string(),
        "HELLO WORLD".to_string(),
    ];

    let results = engine.search("Hello", &text);

    // With case sensitivity, should only match "Hello World"
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].line, 0);
    assert_eq!(results[0].start_col, 0);
    assert_eq!(results[0].end_col, 5);
}

#[test]
fn test_case_insensitive_search() {
    let mut engine = SearchEngine::new();
    engine.set_case_sensitive(false);

    let text = vec![
        "Hello World".to_string(),
        "hello world".to_string(),
        "HELLO WORLD".to_string(),
    ];

    let results = engine.search("hello", &text);

    // Without case sensitivity, should match all lines
    assert_eq!(results.len(), 3);
}

#[test]
fn test_regex_search() {
    let mut engine = SearchEngine::new();
    engine.set_use_regex(true);

    let text = vec![
        "test123".to_string(),
        "test456".to_string(),
        "testABC".to_string(),
    ];

    let results = engine.search(r"test\d+", &text);

    // Should match lines with digits after "test"
    assert_eq!(results.len(), 2);
}

#[test]
fn test_simple_search() {
    let mut engine = SearchEngine::new();
    let text = vec![
        "first line".to_string(),
        "second line".to_string(),
        "third line".to_string(),
    ];

    let results = engine.search("line", &text);

    // Should find "line" in all three lines
    assert_eq!(results.len(), 3);

    for (i, result) in results.iter().enumerate() {
        assert_eq!(result.line, i);
    }
}

#[test]
fn test_no_matches() {
    let mut engine = SearchEngine::new();
    let text = vec!["hello world".to_string(), "foo bar".to_string()];

    let results = engine.search("xyz", &text);
    assert!(results.is_empty());
}

#[test]
fn test_multiple_matches_same_line() {
    let mut engine = SearchEngine::new();
    let text = vec!["test test test".to_string()];

    let results = engine.search("test", &text);

    // Should find multiple instances of "test" on the same line
    assert_eq!(results.len(), 3);
    assert!(results.iter().all(|r| r.line == 0));
}

#[test]
fn test_empty_search() {
    let mut engine = SearchEngine::new();
    let text = vec!["some text".to_string()];

    let results = engine.search("", &text);
    // Empty search should return no results
    assert!(results.is_empty());
}

#[test]
fn test_search_result_structure() {
    let result = SearchResult {
        line: 5,
        start_col: 10,
        end_col: 15,
        matched_text: "match".to_string(),
    };

    assert_eq!(result.line, 5);
    assert_eq!(result.start_col, 10);
    assert_eq!(result.end_col, 15);
    assert_eq!(result.matched_text, "match");
}
