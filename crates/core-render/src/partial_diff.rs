//! Viewport line hash diff classification (Phase 3 Step 5).
//!
//! Compares the current viewport's line hashes against the `PartialCache`.
//! This step deliberately does **not** emit partial terminal updates yet;
//! the full frame path remains active. We only classify which buffer line
//! indices would require repaint once partial rendering is enabled.
//!
//! Cold start / viewport change policy:
//! * If cache is empty, width changed, or viewport origin changed, treat all
//!   visible lines as changed and rebuild the cache.
//! * Otherwise, only lines whose (len, hash) pair differ are classified.
//!
//! Candidate filtering:
//! * Optional `candidates` slice (buffer line indices) restricts repaint
//!   classification to those lines (plus any cold-start lines). This mirrors
//!   later intersection of dirty lines + cursor lines.
//!
//! Metrics integration (RenderPathMetrics):
//! * `dirty_lines_marked`      += raw candidate count (or full viewport size when None).
//! * `dirty_candidate_lines`   += same value (post initial filtering; no cursor additions yet).
//! * `dirty_lines_repainted`   += number of lines classified as changed.
//!
//! Returns buffer line indices (not relative rows) needing repaint.

use crate::partial_cache::PartialCache;
use crate::partial_metrics::RenderPathMetrics;
use core_model::View;
use core_state::EditorState;

/// Classify changed lines in the active viewport and update the cache & metrics.
pub fn classify_viewport_changes(
    state: &EditorState,
    view: &View,
    width: u16,
    height: u16,
    cache: &mut PartialCache,
    metrics: &RenderPathMetrics,
    candidates: Option<&[usize]>,
) -> Vec<usize> {
    let text_height = height.saturating_sub(1); // reserve 1 row for status
    if text_height == 0 {
        return Vec::new();
    }
    let buf = state.active_buffer();
    let start = view.viewport_first_line;
    let end = (start + text_height as usize).min(buf.line_count());
    let visible = end.saturating_sub(start);
    if visible == 0 {
        // Truncate cache hashes if previously populated.
        cache.line_hashes.clear();
        return Vec::new();
    }

    let cold =
        cache.line_hashes.is_empty() || cache.viewport_start != start || cache.width != width;

    if cold {
        cache.reset(start, width, visible);
    }

    // Build candidate filter (buffer line indices).
    use std::collections::HashSet;
    let candidate_set: Option<HashSet<usize>> = candidates.map(|c| c.iter().copied().collect());

    let raw_candidate_count = candidates.map(|c| c.len()).unwrap_or(visible);
    // Metrics: mark & candidate counts.
    use std::sync::atomic::Ordering::Relaxed;
    metrics
        .dirty_lines_marked
        .fetch_add(raw_candidate_count as u64, Relaxed);
    metrics
        .dirty_candidate_lines
        .fetch_add(raw_candidate_count as u64, Relaxed);

    let mut changed: Vec<usize> = Vec::new();

    // Ensure cache large enough on warm path (hash vector length == visible lines).
    if !cold && cache.line_hashes.len() != visible {
        // Viewport size changed (e.g. resize) but not caught by width check (height changed only).
        // Simplest: treat as cold to avoid partial inconsistencies.
        cache.reset(start, width, visible);
    }

    for row in 0..visible {
        let line_idx = start + row;
        let line_content = buf.line(line_idx).unwrap_or_default();
        let trimmed = if line_content.ends_with('\n') || line_content.ends_with('\r') {
            &line_content[..line_content.len() - 1]
        } else {
            line_content.as_str()
        };
        let h = crate::partial_cache::PartialCache::compute_hash(trimmed);
        if cold {
            cache.push_line(h);
            changed.push(line_idx);
            continue;
        }
        // Warm comparison
        if let Some(existing) = cache.line_hashes.get_mut(row) {
            if existing.hash != h.hash || existing.len != h.len {
                *existing = h;
                // Apply candidate filter if present.
                let include = match &candidate_set {
                    Some(set) => set.contains(&line_idx),
                    None => true,
                };
                if include {
                    changed.push(line_idx);
                }
            }
        } else {
            // Missing entry (viewport grew) – treat as changed.
            cache.push_line(h);
            let include = match &candidate_set {
                Some(set) => set.contains(&line_idx),
                None => true,
            };
            if include {
                changed.push(line_idx);
            }
        }
    }
    // Truncate stale hashes if viewport shrank (rare path; not counted as changes).
    if cache.line_hashes.len() > visible {
        cache.line_hashes.truncate(visible);
    }

    metrics
        .dirty_lines_repainted
        .fetch_add(changed.len() as u64, Relaxed);
    changed
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_model::View;
    use core_model::ViewId;
    use core_state::EditorState;
    use core_text::{Buffer, Position};

    fn mk_state(text: &str) -> (EditorState, View) {
        let st = EditorState::new(Buffer::from_str("test", text).unwrap());
        let view = View::new(ViewId(0), st.active, Position::origin(), 0);
        (st, view)
    }

    #[test]
    fn initial_population_marks_all_changed() {
        let (st, view) = mk_state("a\nb\nc\n");
        let mut cache = PartialCache::new();
        let metrics = RenderPathMetrics::default();
        let changed = classify_viewport_changes(&st, &view, 80, 5, &mut cache, &metrics, None);
        // Ropey treats a final trailing newline as producing an additional empty line.
        // Viewport height (text rows) here is 4 (height=5 minus status line), so all 4
        // lines (including the trailing empty one) are classified as changed on cold start.
        assert_eq!(changed, vec![0, 1, 2, 3]);
        let snap = metrics.snapshot();
        assert_eq!(snap.dirty_lines_repainted, 4);
    }

    #[test]
    fn second_pass_no_changes() {
        let (st, view) = mk_state("a\nb\nc\n");
        let mut cache = PartialCache::new();
        let metrics = RenderPathMetrics::default();
        let _ = classify_viewport_changes(&st, &view, 80, 5, &mut cache, &metrics, None);
        let changed2 = classify_viewport_changes(&st, &view, 80, 5, &mut cache, &metrics, None);
        assert!(changed2.is_empty());
    }

    #[test]
    fn single_line_edit_detected() {
        let (mut st, view) = mk_state("alpha\nbeta\ngamma\n");
        let mut cache = PartialCache::new();
        let metrics = RenderPathMetrics::default();
        // Warm cache
        let _ = classify_viewport_changes(&st, &view, 120, 6, &mut cache, &metrics, None);
        // Edit line 1 (insert X at start)
        {
            let buf = st.active_buffer_mut();
            let mut pos = Position::new(1, 0);
            buf.insert_grapheme(&mut pos, "X");
        }
        let changed = classify_viewport_changes(&st, &view, 120, 6, &mut cache, &metrics, None);
        assert_eq!(changed, vec![1]);
    }

    #[test]
    fn newline_insertion_shifts_lines() {
        let (mut st, view) = mk_state("a\nb\nc\n");
        let mut cache = PartialCache::new();
        let metrics = RenderPathMetrics::default();
        let _ = classify_viewport_changes(&st, &view, 80, 6, &mut cache, &metrics, None);
        // Insert newline before 'b'
        {
            let buf = st.active_buffer_mut();
            let mut pos = Position::new(1, 0);
            buf.insert_newline(&mut pos);
        }
        let changed = classify_viewport_changes(&st, &view, 80, 6, &mut cache, &metrics, None);
        assert!(changed.contains(&1));
        assert!(!changed.is_empty());
    }
}
