use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};

/// Render markdown source lines into formatted plain-text lines for the TUI preview.
/// Minimal styling: headings, emphasis/strong (drop markers), lists, code blocks, blockquotes, hr.
pub fn render_markdown_to_lines(
    src_lines: &[String],
    math_mode: &str,
    large_file_mode: &str,
) -> Vec<String> {
    let source = src_lines.join("\n");

    // Options: enable tables, tasklists, footnotes if needed later; keep default minimal for speed
    let opts = Options::empty();
    let parser = Parser::new_ext(&source, opts);

    let mut out: Vec<String> = Vec::new();
    // No header; render content directly

    // State for list bullets/indices and code blocks
    let mut current_line = String::new();
    let mut list_stack: Vec<(bool, usize)> = Vec::new(); // (ordered?, next_index)
    let mut in_code_block: Option<String> = None; // language
    let mut in_blockquote: usize = 0;

    let flush_line = |out: &mut Vec<String>, current_line: &mut String| {
        out.push(std::mem::take(current_line));
    };

    for ev in parser {
        match ev {
            Event::Start(tag) => match tag {
                Tag::Heading { .. } => {
                    if !current_line.is_empty() {
                        flush_line(&mut out, &mut current_line)
                    }
                }
                Tag::List(start) => {
                    list_stack.push((start.is_some(), start.unwrap_or(1) as usize));
                }
                Tag::Item => {
                    if !current_line.is_empty() {
                        flush_line(&mut out, &mut current_line)
                    }
                    let indent = "  ".repeat(list_stack.len().saturating_sub(1));
                    if let Some((ordered, idx)) = list_stack.last_mut() {
                        if *ordered {
                            current_line.push_str(&format!("{}{}. ", indent, *idx));
                            *idx += 1;
                        } else {
                            current_line.push_str(&format!("{}• ", indent));
                        }
                    } else {
                        current_line.push_str("• ");
                    }
                }
                Tag::BlockQuote(_) => {
                    in_blockquote += 1;
                    if !current_line.is_empty() {
                        flush_line(&mut out, &mut current_line)
                    }
                }
                Tag::CodeBlock(kind) => {
                    if !current_line.is_empty() {
                        flush_line(&mut out, &mut current_line)
                    }
                    let lang = match kind {
                        CodeBlockKind::Fenced(lang) => lang.to_string(),
                        _ => String::new(),
                    };
                    in_code_block = Some(lang);
                }
                _ => {}
            },
            Event::End(tag) => match tag {
                TagEnd::Heading(_) => {
                    // Heading: underline with '=' for top-level, '-' otherwise
                    let text = std::mem::take(&mut current_line).trim().to_string();
                    if !text.is_empty() {
                        out.push(text.clone());
                        let underline = "-".repeat(text.chars().count());
                        out.push(underline);
                        out.push(String::new());
                    }
                }
                TagEnd::List(_) => {
                    let _ = list_stack.pop();
                }
                TagEnd::BlockQuote(_) => {
                    in_blockquote = in_blockquote.saturating_sub(1);
                }
                TagEnd::CodeBlock => {
                    let _ = in_code_block.take();
                    out.push("```".to_string());
                    out.push(String::new());
                }
                _ => {}
            },
            Event::Text(text) => {
                if in_code_block.is_some() {
                    for line in text.lines() {
                        out.push(line.to_string());
                    }
                } else {
                    if in_blockquote > 0 && current_line.is_empty() {
                        current_line.push_str(&"│ ".repeat(in_blockquote));
                    }
                    current_line.push_str(&text);
                }
            }
            Event::Code(code) => {
                // Inline code: wrap with backticks
                current_line.push('`');
                current_line.push_str(&code);
                current_line.push('`');
            }
            Event::SoftBreak => {
                current_line.push(' ');
            }
            Event::HardBreak => {
                flush_line(&mut out, &mut current_line);
            }
            Event::Rule => {
                if !current_line.is_empty() {
                    flush_line(&mut out, &mut current_line)
                }
                out.push("—".repeat(20));
            }
            Event::Html(html) => {
                // Preserve inline math if configured; otherwise, strip HTML
                if math_mode != "off" && (html.contains("$") || html.contains("\\(")) {
                    current_line.push_str(&html);
                }
            }
            _ => {}
        }
    }

    if !current_line.is_empty() {
        out.push(current_line);
    }

    // Large file handling (basic): if disable, keep as-is; if truncate, cap lines
    if large_file_mode == "truncate" {
        const MAX_LINES: usize = 5000;
        if out.len() > MAX_LINES {
            out.truncate(MAX_LINES);
        }
    }

    out
}
