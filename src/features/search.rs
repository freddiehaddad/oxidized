// Search and replace functionality
// This will handle pattern matching, regex, and text search

use log::{debug, info};
use regex::Regex;

pub struct SearchEngine {
    last_search: Option<String>,
    case_sensitive: bool,
    use_regex: bool,
}

impl Default for SearchEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl SearchEngine {
    pub fn new() -> Self {
        Self {
            last_search: None,
            case_sensitive: false,
            use_regex: false,
        }
    }

    pub fn set_case_sensitive(&mut self, case_sensitive: bool) {
        self.case_sensitive = case_sensitive;
    }

    pub fn set_use_regex(&mut self, use_regex: bool) {
        self.use_regex = use_regex;
    }

    pub fn search(&mut self, pattern: &str, text: &[String]) -> Vec<SearchResult> {
        info!(
            "Performing search for pattern: '{}' (case_sensitive: {}, use_regex: {})",
            pattern, self.case_sensitive, self.use_regex
        );
        self.last_search = Some(pattern.to_string());

        let mut results = Vec::new();

        // Handle empty pattern
        if pattern.is_empty() {
            return results;
        }

        if self.use_regex {
            if let Ok(regex) = Regex::new(pattern) {
                for (line_num, line) in text.iter().enumerate() {
                    for mat in regex.find_iter(line) {
                        results.push(SearchResult {
                            line: line_num,
                            start_col: mat.start(),
                            end_col: mat.end(),
                            matched_text: mat.as_str().to_string(),
                        });
                    }
                }
            }
        } else {
            // Simple string search with proper UTF-8 handling
            // Precompute pattern chars once
            let pattern_chars: Vec<char> = pattern.chars().collect();
            let pattern_search_chars: Vec<char> = if self.case_sensitive {
                pattern_chars.clone()
            } else {
                pattern.to_lowercase().chars().collect()
            };

            for (line_num, line) in text.iter().enumerate() {
                // Original chars for correct column indices and matched_text extraction
                let line_chars: Vec<char> = line.chars().collect();

                // Create a search view depending on case sensitivity
                let search_chars: std::borrow::Cow<'_, [char]> = if self.case_sensitive {
                    // Borrow without cloning in case-sensitive mode
                    std::borrow::Cow::Borrowed(&line_chars)
                } else {
                    // Lowercase allocation once per line (keep semantics identical to previous version)
                    let lower: Vec<char> = line.to_lowercase().chars().collect();
                    std::borrow::Cow::Owned(lower)
                };

                let mut char_start = 0;
                while char_start + pattern_search_chars.len() <= search_chars.len() {
                    // Check if we have a match at this position
                    let mut matches = true;
                    for (i, &pattern_char) in pattern_search_chars.iter().enumerate() {
                        if search_chars[char_start + i] != pattern_char {
                            matches = false;
                            break;
                        }
                    }

                    if matches {
                        let char_end = char_start + pattern_chars.len();
                        let matched_text: String =
                            line_chars[char_start..char_end].iter().collect();

                        results.push(SearchResult {
                            line: line_num,
                            start_col: char_start,
                            end_col: char_end,
                            matched_text,
                        });
                    }

                    char_start += 1;
                }
            }
        }

        debug!("Search completed, found {} matches", results.len());
        results
    }

    pub fn replace(&self, _pattern: &str, _replacement: &str, _text: &mut [String]) -> usize {
        // TODO: Implement replace functionality
        0
    }
}

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub line: usize,
    pub start_col: usize,
    pub end_col: usize,
    pub matched_text: String,
}
