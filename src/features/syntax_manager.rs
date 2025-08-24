use crate::features::syntax::{HighlightRange, SyntaxHighlighter};
use crate::input::events::EditorEvent;
use crossbeam_channel as xchan;
use log::{debug, trace};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::mpsc::Sender as EventSender;
use tree_sitter::{Point, Tree}; // direct event emission (SyntaxReady)

#[derive(Clone, Debug, Default)]
pub enum LineState {
    #[default]
    Uninitialized,
    Pending {
        requested_version: u64,
        last_ready: Option<Arc<[HighlightRange]>>,
    },
    Ready {
        version: u64,
        spans: Arc<[HighlightRange]>,
    },
    Stale {
        last_ready: Option<Arc<[HighlightRange]>>,
    },
}

#[derive(Default)]
pub struct BufferSyntaxState {
    pub version: u64,
    pub pending_version: u64,
    pub line_states: Vec<LineState>,
    pub language: Option<String>,
}

pub struct SyntaxManager {
    work_tx: xchan::Sender<Work>,
    result_rx: xchan::Receiver<ResultMsg>,
    pub buffers: HashMap<usize, BufferSyntaxState>,
    pub metrics: SyntaxMetrics,
}

#[derive(Debug)]
enum Work {
    ParseAndExtract {
        buffer_id: usize,
        version: u64,
        lines: Vec<usize>,
        full_text: String,
        language: String,
    },
}

#[derive(Debug)]
enum ResultMsg {
    Line {
        buffer_id: usize,
        version: u64,
        line: usize,
        spans: Vec<HighlightRange>,
    },
    LineUnchanged {
        buffer_id: usize,
        version: u64,
        line: usize,
    },
    Metrics {
        incremental: usize,
        fallback: usize,
        full: usize,
        reused: usize,
    },
}

#[derive(Default, Debug, Clone, Copy)]
pub struct SyntaxMetrics {
    pub incremental: usize,
    pub fallback: usize,
    pub full: usize,
    pub reused: usize,
}

impl SyntaxManager {
    pub fn new() -> anyhow::Result<Self> {
        Self::new_with_event_sender(None)
    }

    pub fn new_with_event_sender(
        event_sender: Option<EventSender<EditorEvent>>,
    ) -> anyhow::Result<Self> {
        let highlighter = SyntaxHighlighter::new()?; // kept for worker clone only
        let (wtx, wrx) = xchan::unbounded();
        let (rtx, rrx) = xchan::unbounded();
        let evt_clone = event_sender.clone();
        std::thread::spawn(move || worker_loop(highlighter, wrx, rtx, evt_clone));
        Ok(Self {
            work_tx: wtx,
            result_rx: rrx,
            buffers: HashMap::new(),
            metrics: SyntaxMetrics::default(),
        })
    }

    pub fn buffer_mut(&mut self, buffer_id: usize, line_count: usize) -> &mut BufferSyntaxState {
        let entry = self
            .buffers
            .entry(buffer_id)
            .or_insert_with(|| BufferSyntaxState {
                version: 0,
                pending_version: 0,
                line_states: Vec::new(),
                language: None,
            });
        if entry.line_states.len() < line_count {
            entry
                .line_states
                .resize(line_count, LineState::Uninitialized);
        }
        entry
    }

    pub fn notify_edit(&mut self, buffer_id: usize, line_count: usize) {
        let state = self.buffer_mut(buffer_id, line_count);
        state.version = state.version.wrapping_add(1);
        // Mark all lines stale for now (simple phase); incremental diff later
        for ls in &mut state.line_states {
            *ls = match std::mem::take(ls) {
                LineState::Ready { spans, .. } => LineState::Stale {
                    last_ready: Some(spans),
                },
                LineState::Pending { last_ready, .. } => LineState::Stale { last_ready },
                other => other,
            };
        }
    }

    /// Mark a single line stale without bumping version (used for non-structural inserts)
    pub fn notify_line_changed(&mut self, buffer_id: usize, line_index: usize, line_count: usize) {
        let state = self.buffer_mut(buffer_id, line_count);
        if line_index < state.line_states.len() {
            state.line_states[line_index] = match std::mem::take(&mut state.line_states[line_index])
            {
                LineState::Ready { spans, .. } => LineState::Stale {
                    last_ready: Some(spans),
                },
                LineState::Pending { last_ready, .. } => LineState::Stale { last_ready },
                other => other,
            };
        }
    }

    pub fn ensure_lines(
        &mut self,
        buffer_id: usize,
        line_indices: &[usize],
        full_text: &str,
        language: &str,
    ) {
        let mut need: Vec<usize> = Vec::new();
        let version: u64;
        {
            let line_count = full_text.lines().count().max(1);
            let state = self.buffer_mut(buffer_id, line_count);
            state.language = Some(language.to_string());
            version = state.version;
            for &li in line_indices {
                if li >= state.line_states.len() {
                    continue;
                }
                let schedule = match &state.line_states[li] {
                    LineState::Ready { version, .. } if *version == state.version => false,
                    LineState::Pending {
                        requested_version, ..
                    } if *requested_version == state.version => false,
                    _ => true,
                };
                if schedule {
                    let prev = match std::mem::replace(
                        &mut state.line_states[li],
                        LineState::Pending {
                            requested_version: state.version,
                            last_ready: None,
                        },
                    ) {
                        LineState::Ready { spans, .. } => Some(spans),
                        LineState::Stale { last_ready } => last_ready,
                        LineState::Pending { last_ready, .. } => last_ready,
                        _ => None,
                    };
                    if let LineState::Pending { last_ready, .. } = &mut state.line_states[li] {
                        *last_ready = prev;
                    }
                    need.push(li);
                }
            }
        } // borrow ends here
        if !need.is_empty() {
            let _ = self.work_tx.send(Work::ParseAndExtract {
                buffer_id,
                version,
                lines: need,
                full_text: full_text.to_string(),
                language: language.to_string(),
            });
        }
    }

    pub fn poll_results(&mut self) -> bool {
        let mut updated = false;
        while let Ok(msg) = self.result_rx.try_recv() {
            match msg {
                ResultMsg::Line {
                    buffer_id,
                    version,
                    line,
                    spans,
                } => {
                    if let Some(buf) = self.buffers.get_mut(&buffer_id) {
                        if version == buf.version && line < buf.line_states.len() {
                            // If spans empty and we have previous, keep previous (avoid locking-in empty)
                            if spans.is_empty() {
                                match &mut buf.line_states[line] {
                                    LineState::Pending {
                                        last_ready: Some(prev),
                                        ..
                                    } => {
                                        // Revert to stale so it can be retried later
                                        buf.line_states[line] = LineState::Stale {
                                            last_ready: Some(prev.clone()),
                                        };
                                    }
                                    LineState::Pending {
                                        last_ready: None, ..
                                    } => {
                                        // Leave as stale with None
                                        buf.line_states[line] =
                                            LineState::Stale { last_ready: None };
                                    }
                                    other => {
                                        *other = LineState::Stale { last_ready: None };
                                    }
                                }
                            } else {
                                let arc: Arc<[HighlightRange]> = spans.into();
                                buf.line_states[line] = LineState::Ready {
                                    version,
                                    spans: arc,
                                };
                            }
                            updated = true;
                        } else {
                            trace!(
                                "Dropping stale line result buffer={} line={} version={}",
                                buffer_id, line, version
                            );
                        }
                    }
                }
                ResultMsg::LineUnchanged {
                    buffer_id,
                    version,
                    line,
                } => {
                    if let Some(buf) = self.buffers.get_mut(&buffer_id)
                        && version == buf.version
                        && line < buf.line_states.len()
                    {
                        // Promote pending line to Ready by reusing previous spans.
                        if let LineState::Pending {
                            last_ready: Some(prev),
                            ..
                        } = &buf.line_states[line]
                        {
                            let arc = prev.clone();
                            buf.line_states[line] = LineState::Ready {
                                version,
                                spans: arc,
                            };
                            updated = true;
                        } else if let LineState::Pending {
                            last_ready: None, ..
                        } = &buf.line_states[line]
                        {
                            // No previous spans, mark stale so it can be retried later.
                            buf.line_states[line] = LineState::Stale { last_ready: None };
                        }
                    }
                }
                ResultMsg::Metrics {
                    incremental,
                    fallback,
                    full,
                    reused,
                } => {
                    self.metrics.incremental = incremental;
                    self.metrics.fallback = fallback;
                    self.metrics.full = full;
                    self.metrics.reused = reused;
                }
            }
        }
        updated
    }

    pub fn get_line(&self, buffer_id: usize, line_index: usize) -> Option<Arc<[HighlightRange]>> {
        self.buffers
            .get(&buffer_id)
            .and_then(|b| b.line_states.get(line_index))
            .and_then(|ls| match ls {
                LineState::Ready { spans, .. } => Some(spans.clone()),
                LineState::Pending {
                    last_ready: Some(prev),
                    ..
                } => Some(prev.clone()),
                LineState::Stale {
                    last_ready: Some(prev),
                } => Some(prev.clone()),
                _ => None,
            })
    }

    /// Invalidate all ready lines to force re-highlight (e.g., after theme change)
    pub fn invalidate_all(&mut self) {
        for (_bid, state) in self.buffers.iter_mut() {
            for ls in &mut state.line_states {
                *ls = match std::mem::take(ls) {
                    LineState::Ready { spans, .. } => LineState::Stale {
                        last_ready: Some(spans),
                    },
                    LineState::Pending { last_ready, .. } => LineState::Stale { last_ready },
                    other => other,
                }
            }
        }
    }
}

// Unified provisional delimiter injection: adds missing single-character delimiter spans for requested lines only.
fn add_provisional_delimiters(
    spans: &mut Vec<HighlightRange>,
    full_text: &str,
    language: &str,
    line_starts: &[usize],
    requested_lines: &[usize],
) {
    if language != "rust" {
        return;
    }
    // Derive a fallback style from any existing single-char span or earliest span.
    let fallback_style = spans
        .iter()
        .find(|s| s.end - s.start == 1)
        .or_else(|| spans.first())
        .map(|s| s.style.clone());
    let Some(style) = fallback_style else {
        return;
    };

    // Build interval set for quick coverage test (spans assumed sorted non-overlapping)
    // We'll binary search later; ensure sorting.
    spans.sort_by_key(|s| s.start);

    for &li in requested_lines {
        if li + 1 >= line_starts.len() {
            continue;
        }
        let line_start = line_starts[li];
        let line_end = line_starts[li + 1].saturating_sub(1);
        if line_start >= full_text.len() || line_start >= line_end {
            continue;
        }
        let slice = &full_text[line_start..line_end];
        for (off, ch) in slice.char_indices() {
            if matches!(ch, '{' | '}' | '(' | ')') {
                let global_pos = line_start + off;
                // Coverage check via binary search
                let covered = match spans.binary_search_by_key(&global_pos, |s| s.start) {
                    Ok(_) => true,
                    Err(idx) => {
                        if idx > 0 {
                            let prev = &spans[idx - 1];
                            global_pos < prev.end
                        } else {
                            false
                        }
                    }
                };
                if !covered {
                    spans.push(HighlightRange {
                        start: global_pos,
                        end: global_pos + ch.len_utf8(),
                        style: style.clone(),
                    });
                }
            }
        }
    }
    spans.sort_by_key(|s| (s.start, s.end));
}

fn worker_loop(
    mut highlighter: SyntaxHighlighter,
    wrx: xchan::Receiver<Work>,
    rtx: xchan::Sender<ResultMsg>,
    event_sender: Option<EventSender<EditorEvent>>,
) {
    // Cache per buffer: last full text + parse tree + collected global spans
    struct ParseCache {
        text: String,
        tree: Tree,
        language: String,
        spans: Vec<HighlightRange>,
    }
    let mut parse_cache: HashMap<usize, ParseCache> = HashMap::new();

    // Worker metrics (sent periodically)
    let mut incremental_count: usize = 0;
    let mut fallback_count: usize = 0;
    let mut full_count: usize = 0;
    let mut reused_count: usize = 0;

    fn compute_single_edit(old: &str, new: &str) -> Option<(usize, usize, usize, usize)> {
        if old == new {
            return None;
        }
        let mut start = 0usize;
        let old_bytes = old.as_bytes();
        let new_bytes = new.as_bytes();
        let min_len = old_bytes.len().min(new_bytes.len());
        while start < min_len && old_bytes[start] == new_bytes[start] {
            start += 1;
        }
        let mut old_end = old_bytes.len();
        let mut new_end = new_bytes.len();
        while old_end > start && new_end > start && old_bytes[old_end - 1] == new_bytes[new_end - 1]
        {
            old_end -= 1;
            new_end -= 1;
        }
        Some((start, old_end, start, new_end))
    }

    fn byte_to_point(text: &str, byte: usize) -> Point {
        // tree_sitter::Point in 0.25 uses usize fields; count raw bytes between newlines.
        let mut row: usize = 0;
        let mut col: usize = 0;
        let slice = &text[..byte.min(text.len())];
        for b in slice.bytes() {
            if b == b'\n' {
                row += 1;
                col = 0;
            } else {
                col += 1;
            }
        }
        Point { row, column: col }
    }

    while let Ok(work) = wrx.recv() {
        let Work::ParseAndExtract {
            buffer_id,
            version,
            lines,
            full_text,
            language,
        } = work;
        // Parse the entire buffer once to give tree-sitter full context.
        // This improves early recognition of constructs (e.g. function identifiers)
        // compared to per-line isolated parses.
        debug!(
            "syntax_worker: parse_start buffer={} version={} lines_req={} lang={} len={}B",
            buffer_id,
            version,
            lines.len(),
            language,
            full_text.len()
        );
        // Provide changed line tracking & reuse flags.
        struct SpansResult {
            all_spans: Vec<HighlightRange>,
            changed_lines: Option<HashSet<usize>>,
            reused: bool,
        }
        let spans_result: Result<SpansResult, anyhow::Error>;

        // Helper to collect spans from a non-markdown parse tree without reparsing.
        fn collect_spans_from_tree(
            highlighter: &SyntaxHighlighter,
            language: &str,
            tree: &Tree,
        ) -> Vec<HighlightRange> {
            let mut out = Vec::new();
            if let Some(support) = highlighter.language_support.get(language) {
                let mut stack = vec![tree.root_node()];
                while let Some(node) = stack.pop() {
                    let kind = node.kind();
                    if let Some(cat) = support.node_mappings.get(kind)
                        && let Some(color) = highlighter.get_color_for_category(cat)
                    {
                        out.push(HighlightRange {
                            start: node.start_byte(),
                            end: node.end_byte(),
                            style: crate::features::syntax::HighlightStyle::from_color(color),
                        });
                    }
                    for i in 0..node.child_count() {
                        if let Some(ch) = node.child(i) {
                            stack.push(ch);
                        }
                    }
                }
            }
            out
        }

        if language != "markdown" {
            // Attempt incremental reuse
            let parser_opt = highlighter.parsers.get_mut(&language);
            if let Some(parser) = parser_opt {
                if let Some(cache) = parse_cache.get_mut(&buffer_id) {
                    if cache.language == language {
                        if cache.text == full_text {
                            // Reuse existing global spans entirely.
                            reused_count += 1;
                            spans_result = Ok(SpansResult {
                                all_spans: cache.spans.clone(),
                                changed_lines: Some(HashSet::new()),
                                reused: true,
                            });
                        } else if let Some((start_new, old_end, _unused, new_end)) =
                            compute_single_edit(&cache.text, &full_text)
                        {
                            // single contiguous edit
                            let start_byte = start_new;
                            let old_end_byte = old_end;
                            let new_end_byte = new_end;
                            // Apply edit to old tree
                            let edit = tree_sitter::InputEdit {
                                start_byte,
                                old_end_byte,
                                new_end_byte,
                                start_position: byte_to_point(&cache.text, start_byte),
                                old_end_position: byte_to_point(&cache.text, old_end_byte),
                                new_end_position: byte_to_point(&full_text, new_end_byte),
                            };
                            cache.tree.edit(&edit);
                            if let Some(new_tree) = parser.parse(&full_text, Some(&cache.tree)) {
                                // For now, treat all lines as changed (changed_lines=None) until proper mapping implemented.
                                let new_spans =
                                    collect_spans_from_tree(&highlighter, &language, &new_tree);
                                cache.text = full_text.clone();
                                cache.tree = new_tree;
                                cache.spans = new_spans.clone();
                                incremental_count += 1;
                                spans_result = Ok(SpansResult {
                                    all_spans: new_spans,
                                    changed_lines: None,
                                    reused: false,
                                });
                            } else {
                                // Fallback full parse
                                if let Some(new_tree) = parser.parse(&full_text, None) {
                                    let new_spans =
                                        collect_spans_from_tree(&highlighter, &language, &new_tree);
                                    cache.text = full_text.clone();
                                    cache.tree = new_tree;
                                    cache.spans = new_spans.clone();
                                    fallback_count += 1;
                                    spans_result = Ok(SpansResult {
                                        all_spans: new_spans,
                                        changed_lines: None,
                                        reused: false,
                                    });
                                } else {
                                    spans_result = Err(anyhow::anyhow!("parse failed"));
                                }
                            }
                        } else {
                            // No edit (likely identical) -> reuse existing spans quickly by re-highlighting line slices from cache tree
                            // We already handled identical text above, so reachable when diff algorithm failed but content differs more than one edit.
                            if let Some(new_tree) = parser.parse(&full_text, None) {
                                let new_spans =
                                    collect_spans_from_tree(&highlighter, &language, &new_tree);
                                cache.text = full_text.clone();
                                cache.tree = new_tree;
                                cache.spans = new_spans.clone();
                                full_count += 1;
                                spans_result = Ok(SpansResult {
                                    all_spans: new_spans,
                                    changed_lines: None,
                                    reused: false,
                                });
                            } else {
                                spans_result = Err(anyhow::anyhow!("parse failed"));
                            }
                        }
                    } else {
                        // Language changed for this buffer id; drop cache
                        parse_cache.remove(&buffer_id);
                        let res = highlighter.highlight_text(&full_text, &language).map(|v| {
                            SpansResult {
                                all_spans: v,
                                changed_lines: None,
                                reused: false,
                            }
                        });
                        if res.is_ok() {
                            full_count += 1;
                        }
                        spans_result = res;
                    }
                } else {
                    // First time: parse and cache
                    let res = highlighter.highlight_text(&full_text, &language);
                    if res.is_ok() {
                        full_count += 1;
                    }
                    spans_result = res.map(|v| SpansResult {
                        all_spans: v.clone(),
                        changed_lines: None,
                        reused: false,
                    });
                    // Cache tree only if non-markdown and parse succeeded; highlight_text parsed internally so we must reparse to get tree
                    if language != "markdown"
                        && let Some(parser) = highlighter.parsers.get_mut(&language)
                        && let Some(tree) = parser.parse(&full_text, None)
                        && let Ok(SpansResult { all_spans, .. }) = &spans_result
                    {
                        parse_cache.insert(
                            buffer_id,
                            ParseCache {
                                text: full_text.clone(),
                                tree,
                                language: language.clone(),
                                spans: all_spans.clone(),
                            },
                        );
                    }
                }
            } else {
                let res = highlighter
                    .highlight_text(&full_text, &language)
                    .map(|v| SpansResult {
                        all_spans: v,
                        changed_lines: None,
                        reused: false,
                    });
                if res.is_ok() {
                    full_count += 1;
                }
                spans_result = res;
            }
        } else {
            // markdown path - leave as before
            let res = highlighter
                .highlight_text(&full_text, &language)
                .map(|v| SpansResult {
                    all_spans: v,
                    changed_lines: None,
                    reused: false,
                });
            if res.is_ok() {
                full_count += 1;
            }
            spans_result = res;
        }

        // Precompute line start offsets for mapping global spans to per-line spans.
        // Include a sentinel end offset.
        let mut line_starts: Vec<usize> = Vec::with_capacity(full_text.lines().count() + 1);
        let mut offset = 0;
        for line in full_text.lines() {
            line_starts.push(offset);
            // +1 for the newline that was split (except perhaps last line; treat uniformly)
            offset += line.len() + 1;
        }
        line_starts.push(full_text.len() + 1);

        match spans_result {
            Ok(mut sr) => {
                let total_spans = sr.all_spans.len();
                // Ensure ordering & basic validity
                sr.all_spans.retain(|s| s.end > s.start);
                // First, sort by start then by length (shorter first) so leaf tokens win over container nodes
                sr.all_spans.sort_by_key(|s| (s.start, s.end - s.start));
                // Deduplicate overlapping regions: keep first (shortest) span covering any byte range
                let mut dedup: Vec<HighlightRange> = Vec::with_capacity(sr.all_spans.len());
                for span in sr.all_spans.into_iter() {
                    // If this span overlaps any kept span, skip it (since kept are shorter or equal length at same start)
                    if dedup
                        .iter()
                        .any(|k| !(span.end <= k.start || span.start >= k.end))
                    {
                        continue;
                    }
                    dedup.push(span);
                }
                // Re-sort final spans by (start,end) for downstream assumptions
                dedup.sort_by_key(|s| (s.start, s.end));
                sr.all_spans = dedup;
                // Build a quick lookup for existing span coverage to avoid duplicate provisional braces
                // (We work in global coordinates.)
                // Provisional pass: if braces/parens not yet produced by parser (in incomplete constructs), add them.
                if language == "rust" {
                    add_provisional_delimiters(
                        &mut sr.all_spans,
                        &full_text,
                        &language,
                        &line_starts,
                        &lines,
                    );
                }

                for li in lines {
                    if li >= line_starts.len().saturating_sub(1) {
                        continue;
                    }
                    // Fast path: if we reused and have no changed lines reported OR changed_lines present and li not in set, mark unchanged.
                    // Determine if this line can be skipped (unchanged) based on reuse and changed_lines list.
                    let unchanged = if sr.reused {
                        // If we reused identical content, every requested line is unchanged; promote via LineUnchanged.
                        true
                    } else if let Some(changed) = &sr.changed_lines {
                        // Non-empty changed set: skip lines not in it. Empty set => treat all as changed.
                        if changed.is_empty() {
                            false
                        } else {
                            !changed.contains(&li)
                        }
                    } else {
                        false
                    };
                    // If we always mark reused lines unchanged, ensure we only do this when previous spans exist; else we must emit spans.
                    let force_emit = sr.reused && sr.changed_lines.is_none();
                    if unchanged && !force_emit {
                        let _ = rtx.send(ResultMsg::LineUnchanged {
                            buffer_id,
                            version,
                            line: li,
                        });
                        continue;
                    }
                    let line_start = line_starts[li];
                    let line_end = line_starts[li + 1] - 1; // omit artificial +1 newline

                    // Collect spans overlapping this line and translate to line-relative coordinates
                    let mut line_spans: Vec<HighlightRange> = sr
                        .all_spans
                        .iter()
                        .filter_map(|s| {
                            if s.start >= line_end || s.end <= line_start {
                                return None;
                            }
                            let mut ns = s.clone();
                            // Clip
                            if ns.start < line_start {
                                ns.start = line_start;
                            }
                            if ns.end > line_end {
                                ns.end = line_end;
                            }
                            // Translate to line-relative
                            ns.start -= line_start;
                            ns.end -= line_start;
                            if ns.end > ns.start { Some(ns) } else { None }
                        })
                        .collect();

                    // Local non-overlap guarantee after translation (input already non-overlapping globally)
                    line_spans.sort_by_key(|s| (s.start, s.end));
                    debug!(
                        "syntax_worker: emit buffer={} line={} version={} spans={} total_global_spans={}",
                        buffer_id,
                        li,
                        version,
                        line_spans.len(),
                        total_spans
                    );
                    let _ = rtx.send(ResultMsg::Line {
                        buffer_id,
                        version,
                        line: li,
                        spans: line_spans,
                    });
                }
            }
            Err(e) => {
                debug!(
                    "syntax_worker: parse_error buffer={} version={} err={:?}",
                    buffer_id, version, e
                );
                // Emit empty spans for each requested line (manager will decide whether to keep previous)
                for li in lines {
                    let _ = rtx.send(ResultMsg::Line {
                        buffer_id,
                        version,
                        line: li,
                        spans: Vec::new(),
                    });
                }
            }
        }
        // Emit metrics snapshot after each job
        let _ = rtx.send(ResultMsg::Metrics {
            incremental: incremental_count,
            fallback: fallback_count,
            full: full_count,
            reused: reused_count,
        });
        // Notify UI that syntax results are ready
        if let Some(ev) = &event_sender {
            let _ = ev.send(EditorEvent::SyntaxReady);
        }
    }
}
