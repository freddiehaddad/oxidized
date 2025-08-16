//! Presenter layer for completion items.
//! Shapes, normalizes, and orders completion items independent of providers.

use super::engine::{CompletionContext, CompletionItem};
use super::schema::alias_to_canonical;

pub trait CompletionPresenter: Send + Sync {
    fn present(
        &self,
        all: Vec<CompletionItem>,
        input: &str,
        ctx: Option<&CompletionContext>,
    ) -> Vec<CompletionItem>;
}

#[derive(Default)]
pub struct DefaultPresenter;

impl DefaultPresenter {
    fn normalized_for_match(input_lower: &str) -> (String, Option<&'static str>) {
        if let Some(rest) = input_lower.strip_prefix("setp ") {
            (format!("set {}", rest), Some("setp "))
        } else if input_lower == "setp" || input_lower.starts_with("setp") {
            ("set".to_string(), Some("setp "))
        } else if input_lower.starts_with("set ") || input_lower == "set" {
            (input_lower.to_string(), Some("set "))
        } else {
            (input_lower.to_string(), None)
        }
    }
}

impl CompletionPresenter for DefaultPresenter {
    fn present(
        &self,
        all: Vec<CompletionItem>,
        input: &str,
        _ctx: Option<&CompletionContext>,
    ) -> Vec<CompletionItem> {
        let input_lower = input.to_lowercase();
        let (normalized_for_match, desired_set_prefix) = Self::normalized_for_match(&input_lower);

        // 1) Adjust set vs setp prefix eagerly but do NOT filter out dynamic items here
        let combined: Vec<CompletionItem> = all
            .into_iter()
            .map(|mut item| {
                if let Some(prefix) = desired_set_prefix
                    && prefix == "setp "
                    && item.text.starts_with("set ")
                {
                    item.text = format!("setp {}", &item.text[4..]);
                }
                item
            })
            .collect();

        // 2) Dedup simple ex aliases; prefer canonical w/o "(short)"
        use std::collections::HashMap;
        fn canonical_ex_name(s: &str) -> (String, bool) {
            match s {
                "w" | "write" => ("write".into(), true),
                "q" | "quit" => ("quit".into(), true),
                "q!" | "quit!" => ("quit!".into(), true),
                "x" | "wq" => ("wq".into(), true),
                "e" | "edit" => ("edit".into(), true),
                "b" | "buffer" => ("buffer".into(), true),
                "bn" | "bnext" => ("bnext".into(), true),
                "bp" | "bprev" | "bprevious" => ("bprevious".into(), true),
                "bd" | "bdelete" => ("bdelete".into(), true),
                "bd!" | "bdelete!" => ("bdelete!".into(), true),
                "ls" | "buffers" => ("buffers".into(), true),
                "sp" | "split" => ("split".into(), true),
                "vsp" | "vsplit" => ("vsplit".into(), true),
                "close" => ("close".into(), true),
                "reg" | "registers" => ("registers".into(), true),
                _ => (s.into(), false),
            }
        }
        let mut ex_map: HashMap<String, CompletionItem> = HashMap::new();
        let mut others: Vec<CompletionItem> = Vec::new();
        for item in combined.into_iter() {
            let has_space = item.text.contains(' ');
            if item.category != "set" && !has_space {
                let (canon, is_known) = canonical_ex_name(&item.text);
                if is_known {
                    let entry = ex_map.entry(canon.clone()).or_insert_with(|| item.clone());
                    let entry_is_alias = entry.text != canon;
                    let item_is_canon = item.text == canon;
                    let entry_short = entry.description.to_lowercase().contains("(short)");
                    let item_short = item.description.to_lowercase().contains("(short)");
                    if item_is_canon || (entry_is_alias && !item_short && entry_short) {
                        let mut new_entry = item.clone();
                        new_entry.text = canon;
                        *entry = new_entry;
                    }
                } else {
                    others.push(item);
                }
            } else {
                others.push(item);
            }
        }
        let mut combined: Vec<CompletionItem> = others;
        combined.extend(ex_map.into_values());

        // 3) Map canonical ':set' descriptions to prefer non-short
        use std::collections::HashMap as Map;
        let mut set_canonical_desc: Map<String, String> = Map::new();
        for c in &combined {
            if c.category == "set" && !c.description.to_lowercase().contains("(short)") {
                let key = if let Some(rest) = c.text.strip_prefix("setp ") {
                    format!("set {}", rest)
                } else {
                    c.text.clone()
                };
                set_canonical_desc
                    .entry(key)
                    .or_insert_with(|| c.description.clone());
            }
        }

        // 4) Normalize ':set' alias names; preserve set vs setp
        let prefer_setp = input_lower.starts_with("setp");
        let desired_set_prefix: &str = if prefer_setp { "setp " } else { "set " };
        let mut transformed: Vec<CompletionItem> = Vec::with_capacity(combined.len());
        for mut item in combined.into_iter() {
            if item.category != "set" {
                transformed.push(item);
                continue;
            }
            let after_prefix = item
                .text
                .strip_prefix("setp ")
                .or_else(|| item.text.strip_prefix("set "))
                .unwrap_or(&item.text);
            let mut key = after_prefix;
            let mut tail = "";
            if let Some((k, t)) = after_prefix.split_once(' ') {
                key = k;
                tail = t;
            }
            let is_neg = key.starts_with("no");
            let base = if is_neg { &key[2..] } else { key };
            let canonical = alias_to_canonical(base);
            let new_key = if is_neg {
                format!("no{}", canonical)
            } else {
                canonical.to_string()
            };
            let new_text = if tail.is_empty() {
                format!("{}{}", desired_set_prefix, new_key)
            } else {
                format!("{}{} {}", desired_set_prefix, new_key, tail)
            };
            let desc_lookup_key = if tail.is_empty() {
                format!("set {}", new_key)
            } else {
                format!("set {} ", new_key)
            };
            if let Some(desc) = set_canonical_desc.get(&desc_lookup_key) {
                item.description = desc.clone();
            }
            item.text = new_text;
            transformed.push(item);
        }
        let mut combined: Vec<CompletionItem> = transformed;

        // 5) Deduplicate by text
        combined.sort_by(|a, b| a.text.cmp(&b.text));
        combined.dedup_by(|a, b| a.text == b.text);

        // 6) Alias/negative filtering for ':set'
        let input_is_negative = normalized_for_match.starts_with("set no");
        if normalized_for_match.starts_with("set ") || normalized_for_match.starts_with("setp ") {
            use std::collections::HashSet;
            let mut seen: HashSet<String> = HashSet::new();
            combined.retain(|item| {
                if item.category != "set" {
                    return true;
                }
                if item.text.contains('=') || item.text.ends_with('?') {
                    return true;
                }
                let is_negative_item = item
                    .text
                    .strip_prefix("setp ")
                    .or_else(|| item.text.strip_prefix("set "))
                    .map(|s| s.trim_start().starts_with("no"))
                    .unwrap_or(false);
                if !input_is_negative && is_negative_item {
                    return false;
                }
                let raw_key = item
                    .text
                    .strip_prefix("setp ")
                    .or_else(|| item.text.strip_prefix("set "))
                    .unwrap_or(&item.text)
                    .trim();
                let (is_neg, remainder) = if let Some(rest) = raw_key.strip_prefix("no") {
                    (true, rest)
                } else {
                    (false, raw_key)
                };
                let canonical_pos = alias_to_canonical(remainder);
                let key = if is_neg {
                    if input_is_negative {
                        format!("neg:{}", canonical_pos)
                    } else {
                        format!("pos:{}", canonical_pos)
                    }
                } else {
                    format!("pos:{}", canonical_pos)
                };
                if seen.contains(&key) {
                    false
                } else {
                    seen.insert(key);
                    true
                }
            });

            if !input_lower.ends_with('?') {
                let mut plain_canon: std::collections::HashSet<String> =
                    std::collections::HashSet::new();
                for item in combined
                    .iter()
                    .filter(|c| c.category == "set" && !c.text.ends_with('?'))
                {
                    if let Some(raw) = item
                        .text
                        .strip_prefix("setp ")
                        .or_else(|| item.text.strip_prefix("set "))
                    {
                        let raw = raw.trim();
                        let is_neg = raw.starts_with("no");
                        let key_part = if is_neg { &raw[2..] } else { raw };
                        let canonical = alias_to_canonical(key_part);
                        let canon_key = if is_neg {
                            format!("neg:{}", canonical)
                        } else {
                            format!("pos:{}", canonical)
                        };
                        plain_canon.insert(canon_key);
                    }
                }
                combined.retain(|c| {
                    if c.category != "set" || !c.text.ends_with('?') {
                        return true;
                    }
                    if let Some(raw) = c
                        .text
                        .strip_prefix("setp ")
                        .or_else(|| c.text.strip_prefix("set "))
                    {
                        let raw = raw.trim_end_matches('?').trim();
                        let is_neg = raw.starts_with("no");
                        let key_part = if is_neg { &raw[2..] } else { raw };
                        let canonical = alias_to_canonical(key_part);
                        let canon_key = if is_neg {
                            format!("neg:{}", canonical)
                        } else {
                            format!("pos:{}", canonical)
                        };
                        return !plain_canon.contains(&canon_key);
                    }
                    true
                });
            }
        }

        // 7) Final filter by verb only to keep provider-generated variants
        let input_verb = input_lower.split_whitespace().next().unwrap_or("");
        if input_verb == "set" || input_verb == "setp" {
            let desired = if input_verb == "setp" {
                "setp "
            } else {
                "set "
            };
            combined.retain(|i| i.text.to_lowercase().starts_with(desired));
        } else if !input_verb.is_empty() {
            combined.retain(|i| {
                let item_verb = i
                    .text
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .to_lowercase();
                item_verb.starts_with(input_verb)
            });
        }

        // 8) Sort by length then alpha
        combined.sort_by(|a, b| {
            a.text
                .len()
                .cmp(&b.text.len())
                .then_with(|| a.text.cmp(&b.text))
        });

        combined
    }
}
