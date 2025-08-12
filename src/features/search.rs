// Search and replace functionality
// This will handle pattern matching, regex, and text search

use log::{debug, warn};
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
        // Searches can be frequent; keep at debug to avoid log noise at info
        debug!(
            "Searching for pattern: '{}' (case_sensitive: {}, use_regex: {})",
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
            } else {
                warn!("Invalid regex pattern: '{}'", pattern);
            }
        } else {
            // Simple string search with proper UTF-8 handling
            // Precompute pattern chars once
            let pattern_chars: Vec<char> = pattern.chars().collect();
            let pattern_is_ascii = pattern.is_ascii();
            let pattern_search_chars: Vec<char> = if self.case_sensitive {
                pattern_chars.clone()
            } else if pattern_is_ascii {
                // ASCII fast path: lower via ASCII mapping (same semantics for ASCII)
                pattern_chars
                    .iter()
                    .map(|c| c.to_ascii_lowercase())
                    .collect()
            } else {
                // Unicode case-insensitive: keep previous semantics
                pattern.to_lowercase().chars().collect()
            };

            // Fast path: single-character pattern (and no Unicode expansion)
            if pattern_search_chars.len() == 1 && pattern_chars.len() == 1 {
                let p_cs = pattern_chars[0];
                let p_ci_ascii = pattern_search_chars[0];

                for (line_num, line) in text.iter().enumerate() {
                    let line_chars: Vec<char> = line.chars().collect();

                    if self.case_sensitive {
                        for (idx, &ch) in line_chars.iter().enumerate() {
                            if ch == p_cs {
                                let matched_text: String =
                                    line_chars[idx..idx + 1].iter().collect();
                                results.push(SearchResult {
                                    line: line_num,
                                    start_col: idx,
                                    end_col: idx + 1,
                                    matched_text,
                                });
                            }
                        }
                    } else if pattern_is_ascii && line.is_ascii() {
                        for (idx, &ch) in line_chars.iter().enumerate() {
                            if ch.to_ascii_lowercase() == p_ci_ascii {
                                let matched_text: String =
                                    line_chars[idx..idx + 1].iter().collect();
                                results.push(SearchResult {
                                    line: line_num,
                                    start_col: idx,
                                    end_col: idx + 1,
                                    matched_text,
                                });
                            }
                        }
                    } else {
                        // Fallback to Unicode-insensitive general path for non-ASCII scenarios
                        let search_chars: Vec<char> = if self.case_sensitive {
                            line_chars.clone()
                        } else {
                            line.to_lowercase().chars().collect()
                        };

                        let mut char_start = 0;
                        while char_start + pattern_search_chars.len() <= search_chars.len() {
                            if search_chars[char_start] == p_ci_ascii {
                                let matched_text: String =
                                    line_chars[char_start..char_start + 1].iter().collect();
                                results.push(SearchResult {
                                    line: line_num,
                                    start_col: char_start,
                                    end_col: char_start + 1,
                                    matched_text,
                                });
                            }
                            char_start += 1;
                        }
                    }
                }

                debug!("Search completed, found {} matches", results.len());
                return results;
            }

            for (line_num, line) in text.iter().enumerate() {
                // Original chars for correct column indices and matched_text extraction
                let line_chars: Vec<char> = line.chars().collect();

                if self.case_sensitive {
                    // Case-sensitive: compare directly against original chars without extra allocations
                    let mut char_start = 0;
                    while char_start + pattern_search_chars.len() <= line_chars.len() {
                        let mut matches = true;
                        for (i, &pattern_char) in pattern_search_chars.iter().enumerate() {
                            if line_chars[char_start + i] != pattern_char {
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
                } else if line.is_ascii() && pattern_is_ascii {
                    // Case-insensitive ASCII fast path: avoid allocating a lowercased line
                    let mut char_start = 0;
                    while char_start + pattern_search_chars.len() <= line_chars.len() {
                        let mut matches = true;
                        for (i, &pattern_char) in pattern_search_chars.iter().enumerate() {
                            if line_chars[char_start + i].to_ascii_lowercase() != pattern_char {
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
                } else {
                    // Unicode case-insensitive: allocate a lowercased view per line (previous semantics)
                    let search_chars: Vec<char> = line.to_lowercase().chars().collect();

                    let mut char_start = 0;
                    while char_start + pattern_search_chars.len() <= search_chars.len() {
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
        }

        debug!("Search completed, found {} matches", results.len());
        results
    }

    pub fn replace(&self, pattern: &str, replacement: &str, text: &mut [String]) -> usize {
        // No-op on empty pattern
        if pattern.is_empty() {
            return 0;
        }

        let mut total_replacements = 0usize;

        if self.use_regex {
            // Build regex, honoring case sensitivity via inline flag
            let pat = if self.case_sensitive {
                pattern.to_string()
            } else {
                format!("(?i){}", pattern)
            };
            if let Ok(regex) = Regex::new(&pat) {
                for line in text.iter_mut() {
                    // Count matches first to update total_replacements accurately
                    let count = regex.find_iter(line.as_str()).count();
                    if count > 0 {
                        let replaced = regex.replace_all(line.as_str(), replacement);
                        *line = replaced.into_owned();
                        total_replacements += count;
                    }
                }
            }
            return total_replacements;
        }

        // Non-regex path: Unicode-aware char-by-char replacement matching search() semantics
        let pattern_chars: Vec<char> = pattern.chars().collect();
        let pattern_is_ascii = pattern.is_ascii();
        let pattern_search_chars: Vec<char> = if self.case_sensitive {
            pattern_chars.clone()
        } else if pattern_is_ascii {
            pattern_chars
                .iter()
                .map(|c| c.to_ascii_lowercase())
                .collect()
        } else {
            pattern.to_lowercase().chars().collect()
        };

        for line in text.iter_mut() {
            if line.is_empty() {
                continue;
            }

            let line_chars: Vec<char> = line.chars().collect();
            let mut out = String::with_capacity(line.len());

            if self.case_sensitive {
                let mut i = 0usize;
                while i + pattern_search_chars.len() <= line_chars.len() {
                    let mut matches = true;
                    for (j, &p) in pattern_search_chars.iter().enumerate() {
                        if line_chars[i + j] != p {
                            matches = false;
                            break;
                        }
                    }
                    if matches {
                        out.push_str(replacement);
                        i += pattern_chars.len();
                        total_replacements += 1;
                    } else {
                        out.push(line_chars[i]);
                        i += 1;
                    }
                }
                // Remainder
                while i < line_chars.len() {
                    out.push(line_chars[i]);
                    i += 1;
                }
            } else if line.is_ascii() && pattern_is_ascii {
                let mut i = 0usize;
                while i + pattern_search_chars.len() <= line_chars.len() {
                    let mut matches = true;
                    for (j, &p) in pattern_search_chars.iter().enumerate() {
                        if line_chars[i + j].to_ascii_lowercase() != p {
                            matches = false;
                            break;
                        }
                    }
                    if matches {
                        out.push_str(replacement);
                        i += pattern_chars.len();
                        total_replacements += 1;
                    } else {
                        out.push(line_chars[i]);
                        i += 1;
                    }
                }
                while i < line_chars.len() {
                    out.push(line_chars[i]);
                    i += 1;
                }
            } else {
                // Unicode-insensitive: compare lowercased view
                let search_chars: Vec<char> = line.to_lowercase().chars().collect();
                let mut i = 0usize;
                while i + pattern_search_chars.len() <= search_chars.len() {
                    let mut matches = true;
                    for (j, &p) in pattern_search_chars.iter().enumerate() {
                        if search_chars[i + j] != p {
                            matches = false;
                            break;
                        }
                    }
                    if matches {
                        out.push_str(replacement);
                        i += pattern_chars.len();
                        total_replacements += 1;
                    } else {
                        out.push(line_chars[i]);
                        i += 1;
                    }
                }
                while i < line_chars.len() {
                    out.push(line_chars[i]);
                    i += 1;
                }
            }

            *line = out;
        }

        total_replacements
    }
}

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub line: usize,
    pub start_col: usize,
    pub end_col: usize,
    pub matched_text: String,
}
