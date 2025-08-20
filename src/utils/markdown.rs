use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use std::collections::HashMap;

use crate::features::syntax::SemanticCategory;

#[derive(Debug, Clone)]
pub struct MarkdownSpan {
    pub start: usize,
    pub end: usize,
    pub category: SemanticCategory,
}

#[derive(Debug, Clone)]
pub struct MarkdownRender {
    pub lines: Vec<String>,
    pub spans: HashMap<usize, Vec<MarkdownSpan>>, // line_index -> spans
    /// Mapping from source line index -> preview line index (best effort)
    /// This helps align scroll positions between the source buffer and the preview.
    pub src_to_preview: Vec<usize>,
}

fn flush_current_line(
    out: &mut Vec<String>,
    spans_map: &mut HashMap<usize, Vec<MarkdownSpan>>,
    current_line: &mut String,
    current_spans: &mut Vec<MarkdownSpan>,
    line_index: &mut usize,
) {
    out.push(std::mem::take(current_line));
    if !current_spans.is_empty() {
        spans_map.insert(*line_index, std::mem::take(current_spans));
    }
    *line_index += 1;
}

/// Render markdown into formatted lines + semantic spans.
pub fn render_markdown(
    src_lines: &[String],
    math_mode: &str,
    large_file_mode: &str,
) -> MarkdownRender {
    let source = src_lines.join("\n");

    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TASKLISTS);
    let parser = Parser::new_ext(&source, opts);

    let mut out: Vec<String> = Vec::new();
    let mut spans: HashMap<usize, Vec<MarkdownSpan>> = HashMap::new();

    let mut current_line = String::new();
    let mut current_spans: Vec<MarkdownSpan> = Vec::new();
    let mut line_index: usize = 0;

    let mut list_stack: Vec<(bool, usize)> = Vec::new(); // (ordered?, next_index)
    let mut in_code_block: Option<String> = None;
    let mut in_blockquote: usize = 0;
    let mut quote_open_count: usize = 0; // tracks outermost quote boundaries for spacing

    // Inline state (normal lines)
    let mut link_stack: Vec<usize> = Vec::new();
    let mut emphasis_stack: Vec<usize> = Vec::new();
    let mut strong_stack: Vec<usize> = Vec::new();

    // Heading capture state
    let mut current_heading_level: Option<u8> = None;
    let mut in_heading: bool = false;
    let mut heading_buf: String = String::new();
    let mut heading_spans_tmp: Vec<MarkdownSpan> = Vec::new();
    let mut heading_link_stack: Vec<usize> = Vec::new();
    let mut heading_emphasis_stack: Vec<usize> = Vec::new();
    let mut heading_strong_stack: Vec<usize> = Vec::new();

    for event in parser {
        match event {
            Event::Start(tag) => match tag {
                Tag::Paragraph => {}
                Tag::List(start) => {
                    list_stack.push((start.is_some(), start.unwrap_or(1) as usize));
                }
                Tag::Item => {
                    if !current_line.is_empty() {
                        flush_current_line(
                            &mut out,
                            &mut spans,
                            &mut current_line,
                            &mut current_spans,
                            &mut line_index,
                        );
                    }
                    if let Some((ordered, next)) = list_stack.last_mut() {
                        if *ordered {
                            let prefix = format!("{}. ", *next);
                            *next += 1;
                            current_line.push_str(&prefix);
                        } else {
                            current_line.push_str("• ");
                        }
                    }
                }
                Tag::BlockQuote(_) => {
                    in_blockquote += 1;
                    // Add breathing room before the first (outermost) blockquote
                    if in_blockquote == 1 {
                        if (!out.is_empty() && out.last().map(|l| !l.is_empty()).unwrap_or(false))
                            || (!current_line.is_empty())
                        {
                            if !current_line.is_empty() {
                                flush_current_line(
                                    &mut out,
                                    &mut spans,
                                    &mut current_line,
                                    &mut current_spans,
                                    &mut line_index,
                                );
                            }
                            out.push(String::new());
                            line_index += 1;
                        }
                        quote_open_count += 1;
                    }
                }
                Tag::CodeBlock(kind) => {
                    if !current_line.is_empty() {
                        flush_current_line(
                            &mut out,
                            &mut spans,
                            &mut current_line,
                            &mut current_spans,
                            &mut line_index,
                        );
                    }
                    // If a code block starts inside a list item, ensure a blank line
                    // between the list item's text line and the code block content.
                    if !list_stack.is_empty() && out.last().map(|l| !l.is_empty()).unwrap_or(false)
                    {
                        out.push(String::new());
                        line_index += 1;
                    }
                    let lang = match kind {
                        CodeBlockKind::Fenced(lang) => lang.to_string(),
                        _ => String::new(),
                    };
                    let _ = lang; // reserved for future use
                    in_code_block = Some(String::new());
                }
                Tag::Heading { level, .. } => {
                    // Ensure spacing before the heading
                    if !current_line.is_empty() {
                        flush_current_line(
                            &mut out,
                            &mut spans,
                            &mut current_line,
                            &mut current_spans,
                            &mut line_index,
                        );
                    }
                    if out.last().map(|l| !l.is_empty()).unwrap_or(false) {
                        out.push(String::new());
                        line_index += 1;
                    }
                    in_heading = true;
                    heading_buf.clear();
                    heading_spans_tmp.clear();
                    heading_link_stack.clear();
                    heading_emphasis_stack.clear();
                    heading_strong_stack.clear();
                    current_heading_level = Some(match level {
                        HeadingLevel::H1 => 1,
                        HeadingLevel::H2 => 2,
                        HeadingLevel::H3 => 3,
                        HeadingLevel::H4 => 4,
                        HeadingLevel::H5 => 5,
                        HeadingLevel::H6 => 6,
                    });
                }
                Tag::Link { .. } => {
                    // Start a bracketed link: remember start and emit opening bracket
                    if in_heading {
                        let start = heading_buf.len();
                        heading_buf.push('[');
                        heading_link_stack.push(start);
                    } else {
                        let start = current_line.len();
                        current_line.push('[');
                        link_stack.push(start);
                    }
                }
                Tag::Emphasis => {
                    if in_heading {
                        heading_emphasis_stack.push(heading_buf.len());
                    } else {
                        emphasis_stack.push(current_line.len());
                    }
                }
                Tag::Strong => {
                    if in_heading {
                        heading_strong_stack.push(heading_buf.len());
                    } else {
                        strong_stack.push(current_line.len());
                    }
                }
                Tag::Image { dest_url, .. } => {
                    // Render a light-weight image indicator + url
                    let start = current_line.len();
                    current_line.push_str("[img]");
                    let end = current_line.len();
                    current_spans.push(MarkdownSpan {
                        start,
                        end,
                        category: SemanticCategory::Attribute,
                    });
                    current_line.push(' ');
                    current_line.push('(');
                    let url_start = current_line.len();
                    current_line.push_str(&dest_url);
                    let url_end = current_line.len();
                    current_spans.push(MarkdownSpan {
                        start: url_start,
                        end: url_end,
                        category: SemanticCategory::Attribute,
                    });
                    current_line.push(')');
                }
                _ => {}
            },
            Event::End(tag) => match tag {
                TagEnd::Heading(_) => {
                    // Build heading from captured buffer to avoid duplicate text
                    in_heading = false;

                    let mut prev_spans = std::mem::take(&mut heading_spans_tmp);
                    let trimmed = heading_buf.trim().to_string();
                    let lvl = current_heading_level.take().unwrap_or(2);

                    if !trimmed.is_empty() {
                        // Build any blockquote prefix that may be active
                        let prefix = if in_blockquote > 0 {
                            "▎ ".repeat(in_blockquote)
                        } else {
                            String::new()
                        };
                        let indent = ""; // do not indent headings

                        // Heading display line: prefix + indent + content
                        current_line =
                            String::with_capacity(prefix.len() + indent.len() + trimmed.len());
                        current_line.push_str(&prefix);
                        current_line.push_str(indent);
                        let heading_text_start = current_line.len();
                        current_line.push_str(&trimmed);

                        // Spans: color prefix (if any) as comment
                        if !prefix.is_empty() {
                            current_spans.push(MarkdownSpan {
                                start: 0,
                                end: prefix.len(),
                                category: SemanticCategory::Comment,
                            });
                        }

                        // Re-apply inline spans captured during heading content, shifted by prefix+indent,
                        // and split the heading 'Type' span to avoid overlaps (which can cause duplicate drawing).
                        let shift = prefix.len() + indent.len();
                        let heading_text_end = current_line.len();
                        let mut specials: Vec<MarkdownSpan> = prev_spans
                            .drain(..)
                            .map(|sp| MarkdownSpan {
                                start: sp.start + shift,
                                end: sp.end + shift,
                                category: sp.category,
                            })
                            .filter(|sp| sp.start < sp.end)
                            .collect();
                        // Clamp and sort specials
                        for sp in specials.iter_mut() {
                            if sp.start < heading_text_start {
                                sp.start = heading_text_start;
                            }
                            if sp.end > heading_text_end {
                                sp.end = heading_text_end;
                            }
                        }
                        specials.sort_by(|a, b| a.start.cmp(&b.start).then(a.end.cmp(&b.end)));

                        let mut cursor = heading_text_start;
                        for sp in specials.into_iter() {
                            if sp.start > cursor {
                                current_spans.push(MarkdownSpan {
                                    start: cursor,
                                    end: sp.start,
                                    category: SemanticCategory::Type,
                                });
                            }
                            let sp_end = sp.end;
                            current_spans.push(sp);
                            if sp_end > cursor {
                                cursor = sp_end;
                            }
                        }
                        if cursor < heading_text_end {
                            current_spans.push(MarkdownSpan {
                                start: cursor,
                                end: heading_text_end,
                                category: SemanticCategory::Type,
                            });
                        }
                        flush_current_line(
                            &mut out,
                            &mut spans,
                            &mut current_line,
                            &mut current_spans,
                            &mut line_index,
                        );

                        // Underline style: '=' for H1, '-' for all other levels
                        let underline_char = if lvl == 1 { '=' } else { '-' };

                        use unicode_width::UnicodeWidthStr;
                        let underline_len = UnicodeWidthStr::width(trimmed.as_str());
                        current_line = String::with_capacity(prefix.len() + underline_len);
                        current_line.push_str(&prefix);
                        let underline_start = current_line.len();
                        current_line.push_str(&underline_char.to_string().repeat(underline_len));

                        if !prefix.is_empty() {
                            current_spans.push(MarkdownSpan {
                                start: 0,
                                end: prefix.len(),
                                category: SemanticCategory::Comment,
                            });
                        }
                        current_spans.push(MarkdownSpan {
                            start: underline_start,
                            end: current_line.len(),
                            category: SemanticCategory::Delimiter,
                        });
                        flush_current_line(
                            &mut out,
                            &mut spans,
                            &mut current_line,
                            &mut current_spans,
                            &mut line_index,
                        );

                        // Separator blank line
                        out.push(String::new());
                        line_index += 1;
                    }
                }
                TagEnd::List(_) => {
                    // When the outermost list ends, ensure a blank line separates it
                    // from any following paragraph or block content.
                    let _ = list_stack.pop();
                    if list_stack.is_empty()
                        && current_line.is_empty()
                        && out.last().map(|l| !l.is_empty()).unwrap_or(false)
                    {
                        out.push(String::new());
                        line_index += 1;
                    }
                }
                TagEnd::Item => {
                    if !current_line.is_empty() {
                        flush_current_line(
                            &mut out,
                            &mut spans,
                            &mut current_line,
                            &mut current_spans,
                            &mut line_index,
                        );
                    }
                }
                TagEnd::Paragraph => {
                    if !current_line.is_empty() {
                        flush_current_line(
                            &mut out,
                            &mut spans,
                            &mut current_line,
                            &mut current_spans,
                            &mut line_index,
                        );
                    }
                    if list_stack.is_empty() && out.last().map(|l| !l.is_empty()).unwrap_or(false) {
                        out.push(String::new());
                        line_index += 1;
                    }
                }
                TagEnd::BlockQuote(_) => {
                    in_blockquote = in_blockquote.saturating_sub(1);
                    if in_blockquote == 0 && quote_open_count > 0 {
                        if out.last().map(|l| !l.is_empty()).unwrap_or(false)
                            || !current_line.is_empty()
                        {
                            if !current_line.is_empty() {
                                flush_current_line(
                                    &mut out,
                                    &mut spans,
                                    &mut current_line,
                                    &mut current_spans,
                                    &mut line_index,
                                );
                            }
                            out.push(String::new());
                            line_index += 1;
                        }
                        quote_open_count = quote_open_count.saturating_sub(1);
                    }
                }
                TagEnd::CodeBlock => {
                    let _ = in_code_block.take();
                    out.push(String::new());
                    line_index += 1;
                }
                TagEnd::Link => {
                    // Close the bracketed link and split spans: '[' and ']' as Punctuation, inner text as Attribute
                    if in_heading {
                        heading_buf.push(']');
                        if let Some(bracket_start) = heading_link_stack.pop() {
                            let right_bracket_pos = heading_buf.len() - 1;
                            let inner_start = bracket_start + 1;
                            let inner_end = right_bracket_pos; // exclusive end for attribute span
                            // Left bracket
                            heading_spans_tmp.push(MarkdownSpan {
                                start: bracket_start,
                                end: bracket_start + 1,
                                category: SemanticCategory::Punctuation,
                            });
                            // Inner text as attribute (only if non-empty)
                            if inner_start < inner_end {
                                heading_spans_tmp.push(MarkdownSpan {
                                    start: inner_start,
                                    end: inner_end,
                                    category: SemanticCategory::Attribute,
                                });
                            }
                            // Right bracket
                            heading_spans_tmp.push(MarkdownSpan {
                                start: right_bracket_pos,
                                end: right_bracket_pos + 1,
                                category: SemanticCategory::Punctuation,
                            });
                        }
                    } else {
                        current_line.push(']');
                        if let Some(bracket_start) = link_stack.pop() {
                            let right_bracket_pos = current_line.len() - 1;
                            let inner_start = bracket_start + 1;
                            let inner_end = right_bracket_pos; // exclusive end for attribute span
                            // Left bracket
                            current_spans.push(MarkdownSpan {
                                start: bracket_start,
                                end: bracket_start + 1,
                                category: SemanticCategory::Punctuation,
                            });
                            // Inner text as attribute (only if non-empty)
                            if inner_start < inner_end {
                                current_spans.push(MarkdownSpan {
                                    start: inner_start,
                                    end: inner_end,
                                    category: SemanticCategory::Attribute,
                                });
                            }
                            // Right bracket
                            current_spans.push(MarkdownSpan {
                                start: right_bracket_pos,
                                end: right_bracket_pos + 1,
                                category: SemanticCategory::Punctuation,
                            });
                        }
                    }
                }
                TagEnd::Emphasis => {
                    if in_heading {
                        if let Some(start) = heading_emphasis_stack.pop() {
                            let end = heading_buf.len();
                            if start < end {
                                // Split around link spans (both brackets and inner text) to avoid overlap
                                let mut blocks: Vec<(usize, usize)> = heading_spans_tmp
                                    .iter()
                                    .filter(|sp| {
                                        (sp.category == SemanticCategory::Attribute
                                            || sp.category == SemanticCategory::Punctuation)
                                            && sp.end > start
                                            && sp.start < end
                                    })
                                    .map(|sp| (sp.start, sp.end))
                                    .collect();
                                blocks.sort_by_key(|r| r.0);
                                let mut cursor = start;
                                for (b_start, b_end) in blocks {
                                    if b_start > cursor {
                                        heading_spans_tmp.push(MarkdownSpan {
                                            start: cursor,
                                            end: b_start.min(end),
                                            category: SemanticCategory::String,
                                        });
                                    }
                                    cursor = cursor.max(b_end);
                                    if cursor >= end {
                                        break;
                                    }
                                }
                                if cursor < end {
                                    heading_spans_tmp.push(MarkdownSpan {
                                        start: cursor,
                                        end,
                                        category: SemanticCategory::String,
                                    });
                                }
                            }
                        }
                    } else if let Some(start) = emphasis_stack.pop() {
                        let end = current_line.len();
                        if start < end {
                            // Split around link spans (both brackets and inner text) to avoid overlap
                            let mut blocks: Vec<(usize, usize)> = current_spans
                                .iter()
                                .filter(|sp| {
                                    (sp.category == SemanticCategory::Attribute
                                        || sp.category == SemanticCategory::Punctuation)
                                        && sp.end > start
                                        && sp.start < end
                                })
                                .map(|sp| (sp.start, sp.end))
                                .collect();
                            blocks.sort_by_key(|r| r.0);
                            let mut cursor = start;
                            for (b_start, b_end) in blocks {
                                if b_start > cursor {
                                    current_spans.push(MarkdownSpan {
                                        start: cursor,
                                        end: b_start.min(end),
                                        category: SemanticCategory::String,
                                    });
                                }
                                cursor = cursor.max(b_end);
                                if cursor >= end {
                                    break;
                                }
                            }
                            if cursor < end {
                                current_spans.push(MarkdownSpan {
                                    start: cursor,
                                    end,
                                    category: SemanticCategory::String,
                                });
                            }
                        }
                    }
                }
                TagEnd::Strong => {
                    if in_heading {
                        if let Some(start) = heading_strong_stack.pop() {
                            let end = heading_buf.len();
                            if start < end {
                                // Split around link spans (both brackets and inner text) to avoid overlap
                                let mut blocks: Vec<(usize, usize)> = heading_spans_tmp
                                    .iter()
                                    .filter(|sp| {
                                        (sp.category == SemanticCategory::Attribute
                                            || sp.category == SemanticCategory::Punctuation)
                                            && sp.end > start
                                            && sp.start < end
                                    })
                                    .map(|sp| (sp.start, sp.end))
                                    .collect();
                                blocks.sort_by_key(|r| r.0);
                                let mut cursor = start;
                                for (b_start, b_end) in blocks {
                                    if b_start > cursor {
                                        heading_spans_tmp.push(MarkdownSpan {
                                            start: cursor,
                                            end: b_start.min(end),
                                            category: SemanticCategory::Constant,
                                        });
                                    }
                                    cursor = cursor.max(b_end);
                                    if cursor >= end {
                                        break;
                                    }
                                }
                                if cursor < end {
                                    heading_spans_tmp.push(MarkdownSpan {
                                        start: cursor,
                                        end,
                                        category: SemanticCategory::Constant,
                                    });
                                }
                            }
                        }
                    } else if let Some(start) = strong_stack.pop() {
                        let end = current_line.len();
                        if start < end {
                            // Split around link spans (both brackets and inner text) to avoid overlap
                            let mut blocks: Vec<(usize, usize)> = current_spans
                                .iter()
                                .filter(|sp| {
                                    (sp.category == SemanticCategory::Attribute
                                        || sp.category == SemanticCategory::Punctuation)
                                        && sp.end > start
                                        && sp.start < end
                                })
                                .map(|sp| (sp.start, sp.end))
                                .collect();
                            blocks.sort_by_key(|r| r.0);
                            let mut cursor = start;
                            for (b_start, b_end) in blocks {
                                if b_start > cursor {
                                    current_spans.push(MarkdownSpan {
                                        start: cursor,
                                        end: b_start.min(end),
                                        category: SemanticCategory::Constant,
                                    });
                                }
                                cursor = cursor.max(b_end);
                                if cursor >= end {
                                    break;
                                }
                            }
                            if cursor < end {
                                current_spans.push(MarkdownSpan {
                                    start: cursor,
                                    end,
                                    category: SemanticCategory::Constant,
                                });
                            }
                        }
                    }
                }
                _ => {}
            },
            Event::Text(text) => {
                if in_code_block.is_some() {
                    // Code block: indent lines by 4 spaces, no closing fence
                    for line in text.lines() {
                        let mut rendered = String::with_capacity(4 + line.len());
                        rendered.push_str("    ");
                        let start = 4usize;
                        rendered.push_str(line);
                        let end = rendered.len();
                        out.push(rendered);
                        spans.insert(
                            line_index,
                            vec![MarkdownSpan {
                                start,
                                end,
                                category: SemanticCategory::Comment,
                            }],
                        );
                        line_index += 1;
                    }
                } else if in_heading {
                    heading_buf.push_str(&text);
                } else {
                    if in_blockquote > 0 && current_line.is_empty() {
                        // Quote prefix (styled), prefix only
                        let prefix = "▎ ".repeat(in_blockquote);
                        current_line.push_str(&prefix);
                        let end = current_line.len();
                        current_spans.push(MarkdownSpan {
                            start: 0,
                            end,
                            category: SemanticCategory::Comment,
                        });
                    }
                    current_line.push_str(&text);
                }
            }
            Event::Code(code) => {
                // Inline code: no backticks, highlight content as Comment per request
                if in_heading {
                    let start = heading_buf.len();
                    heading_buf.push_str(&code);
                    let end = heading_buf.len();
                    heading_spans_tmp.push(MarkdownSpan {
                        start,
                        end,
                        category: SemanticCategory::Comment,
                    });
                } else {
                    let start = current_line.len();
                    current_line.push_str(&code);
                    let end = current_line.len();
                    current_spans.push(MarkdownSpan {
                        start,
                        end,
                        category: SemanticCategory::Comment,
                    });
                }
            }
            Event::SoftBreak => {
                // In blockquotes, preserve source line boundaries: break the line
                // so each quoted source line gets its own prefixed preview line.
                if in_blockquote > 0 {
                    flush_current_line(
                        &mut out,
                        &mut spans,
                        &mut current_line,
                        &mut current_spans,
                        &mut line_index,
                    );
                } else {
                    current_line.push(' ');
                }
            }
            Event::HardBreak => {
                flush_current_line(
                    &mut out,
                    &mut spans,
                    &mut current_line,
                    &mut current_spans,
                    &mut line_index,
                );
            }
            Event::Rule => {
                if !current_line.is_empty() {
                    flush_current_line(
                        &mut out,
                        &mut spans,
                        &mut current_line,
                        &mut current_spans,
                        &mut line_index,
                    );
                }
                current_line = "—".repeat(20);
                current_spans.push(MarkdownSpan {
                    start: 0,
                    end: current_line.len(),
                    category: SemanticCategory::Delimiter,
                });
                flush_current_line(
                    &mut out,
                    &mut spans,
                    &mut current_line,
                    &mut current_spans,
                    &mut line_index,
                );
            }
            Event::Html(html) => {
                if math_mode != "off" && (html.contains("$") || html.contains("\\(")) {
                    current_line.push_str(&html);
                }
            }
            Event::TaskListMarker(checked) => {
                let marker = if checked { "[x] " } else { "[ ] " };
                let start = current_line.len();
                current_line.push_str(marker);
                let end = current_line.len();
                current_spans.push(MarkdownSpan {
                    start,
                    end,
                    category: SemanticCategory::Punctuation,
                });
            }
            _ => {}
        }
    }

    if !current_line.is_empty() {
        out.push(current_line);
        if !current_spans.is_empty() {
            spans.insert(line_index, current_spans);
        }
    }

    if large_file_mode == "truncate" {
        const MAX_LINES: usize = 5000;
        if out.len() > MAX_LINES {
            out.truncate(MAX_LINES);
        }
    }

    // Build a best-effort source -> preview line map to keep scroll positions aligned
    let src_to_preview = compute_src_to_preview_line_map(src_lines, &out, math_mode);

    MarkdownRender {
        lines: out,
        spans,
        src_to_preview,
    }
}

/// Heuristic mapping from source lines to preview lines.
/// Handles common drift sources:
/// - Fenced code block fences (``` or ~~~) are not rendered; inner lines map to indented preview lines.
/// - Headings render as text + underline + blank separator (3 preview lines for 1 source line).
/// - Inline HTML is often dropped (unless math_mode is active); such lines map to the next preview line.
/// - Lists and blockquotes add prefixes; we match on content suffix to find corresponding preview lines.
fn compute_src_to_preview_line_map(
    src_lines: &[String],
    preview_lines: &[String],
    math_mode: &str,
) -> Vec<usize> {
    fn is_fence(s: &str) -> bool {
        let t = s.trim_start();
        t.starts_with("```") || t.starts_with("~~~")
    }
    fn is_heading(s: &str) -> Option<String> {
        let t = s.trim_start();
        let mut hashes = 0;
        for c in t.chars() {
            if c == '#' {
                hashes += 1;
            } else {
                break;
            }
        }
        if hashes > 0 {
            let rest = t[hashes..].trim_start();
            if !rest.is_empty() {
                return Some(rest.to_string());
            }
        }
        None
    }
    fn strip_list_prefix(s: &str) -> &str {
        let t = s.trim_start();
        if t.starts_with("- ") || t.starts_with("* ") || t.starts_with("+ ") {
            return &t[2..];
        }
        // ordered list like "12. text"
        let mut idx = 0;
        let bytes = t.as_bytes();
        while idx < bytes.len() && bytes[idx].is_ascii_digit() {
            idx += 1;
        }
        if idx < bytes.len() && bytes[idx] == b'.' {
            let mut j = idx + 1;
            if j < bytes.len() && bytes[j] == b' ' {
                j += 1;
            }
            return &t[j..];
        }
        t
    }
    fn strip_quote_prefix(s: &str) -> &str {
        let mut t = s;
        // Remove any number of ">" and following single space
        loop {
            let ts = t.trim_start();
            if let Some(rest) = ts.strip_prefix('>') {
                t = rest.trim_start();
            } else {
                break;
            }
        }
        t
    }
    fn likely_html_line(s: &str) -> bool {
        let t = s.trim();
        t.contains('<') && t.contains('>')
    }

    let mut map = Vec::with_capacity(src_lines.len());
    let mut j: usize = 0; // pointer into preview lines
    let mut in_code = false;
    let pl = preview_lines.len();
    let mut i = 0usize;
    while i < src_lines.len() {
        let line = &src_lines[i];
        let trimmed = line.trim_end();

        // Clamp a helper
        let clamp_j = |x: usize| x.min(pl.saturating_sub(1));

        if is_fence(trimmed) {
            // Opening or closing fence: does not render
            in_code = !in_code; // toggle state (heuristic)
            map.push(clamp_j(j));
            // If closing, try to step over the blank line that renderer inserts after code blocks
            if !in_code {
                // advance over at most one blank preview line
                if j < pl && preview_lines[j].is_empty() {
                    j = j.saturating_add(1);
                }
            }
            i += 1;
            continue;
        }

        if in_code {
            // Expect an indented preview line: "    {content}"
            let expected = {
                let mut s = String::with_capacity(4 + trimmed.len());
                s.push_str("    ");
                s.push_str(trimmed);
                s
            };
            let k = find_forward(preview_lines, j, &expected, 64);
            let k = k.unwrap_or(j);
            map.push(clamp_j(k));
            j = k.saturating_add(1);
            i += 1;
            continue;
        }

        if let Some(head_text) = is_heading(trimmed) {
            // Find heading text line exactly
            let k = find_forward(preview_lines, j, &head_text, 64)
                .or_else(|| find_contains_forward(preview_lines, j, &head_text, 64))
                .unwrap_or(j);
            map.push(clamp_j(k));
            // Skip underline and blank if present
            j = k.saturating_add(1);
            if j < pl && !preview_lines[j].is_empty() {
                // underline present
                j += 1;
            }
            if j < pl && preview_lines[j].is_empty() {
                j += 1;
            }
            i += 1;
            continue;
        }

        // Inline HTML often removed (unless math enabled and contains math markers)
        if likely_html_line(trimmed)
            && (math_mode == "off" || (!trimmed.contains('$') && !trimmed.contains("\\(")))
        {
            map.push(clamp_j(j));
            i += 1;
            continue;
        }

        // List items and quotes: match on content without prefixes
        let mut content = strip_list_prefix(trimmed);
        content = strip_quote_prefix(content);
        let content = content.trim();

        // Try exact, then contains
        let k = if !content.is_empty() {
            find_forward(preview_lines, j, content, 64)
                .or_else(|| find_contains_forward(preview_lines, j, content, 64))
        } else {
            None
        };
        if let Some(k) = k {
            map.push(clamp_j(k));
            j = k.saturating_add(1);
        } else {
            // If we can't find it, stay at current j (common for soft-wrapped paragraphs merged into one line)
            map.push(clamp_j(j));
        }
        i += 1;
    }

    if map.is_empty() && pl > 0 {
        map.push(0);
    }
    // Ensure non-decreasing mapping to avoid backwards jumps that can cause jitter
    let mut last = 0usize;
    for v in map.iter_mut() {
        if *v < last {
            *v = last;
        }
        last = *v;
    }
    map
}

fn find_forward(hay: &[String], start: usize, needle: &str, max_scan: usize) -> Option<usize> {
    hay.iter()
        .enumerate()
        .skip(start)
        .take(max_scan.min(hay.len().saturating_sub(start)))
        .find(|(_i, s)| s.as_str() == needle)
        .map(|(i, _)| i)
}

fn find_contains_forward(
    hay: &[String],
    start: usize,
    needle: &str,
    max_scan: usize,
) -> Option<usize> {
    if needle.is_empty() {
        return None;
    }
    hay.iter()
        .enumerate()
        .skip(start)
        .take(max_scan.min(hay.len().saturating_sub(start)))
        .find(|(_i, s)| s.contains(needle))
        .map(|(i, _)| i)
}
