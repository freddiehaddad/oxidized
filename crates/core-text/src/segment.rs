//! Centralized normalization + segmentation adapter.
//!
//! Contract:
//! - Input: &str raw input (may be received from IME, paste, etc.)
//! - Output: (normalized NFC String, Vec<Segment>) where each segment is a grapheme cluster
//!   with absolute byte offsets into the normalized string and a display width (terminal cells).
//! - Guarantees: Clusters are in order, non-overlapping, cover the entire string when concatenated.
//! - Safety: Does not log content; callers should avoid logging raw text to adhere to logging policy.

use crate::egc_width;
use unicode_normalization::UnicodeNormalization;
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment {
    pub cluster: String,
    pub start: usize, // byte offset in normalized string (inclusive)
    pub end: usize,   // byte offset in normalized string (exclusive)
    pub width: u16,   // terminal cell width (post width overrides)
}

/// Normalize to NFC and segment into grapheme clusters with widths and byte ranges.
pub fn normalize_and_segment(input: &str) -> (String, Vec<Segment>) {
    let normalized: String = input.nfc().collect();
    let mut out = Vec::new();
    let mut byte = 0usize;
    for g in normalized.graphemes(true) {
        let len = g.len();
        let seg = Segment {
            cluster: g.to_string(),
            start: byte,
            end: byte + len,
            width: egc_width(g),
        };
        out.push(seg);
        byte += len;
    }
    (normalized, out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nfc_equivalence_and_segmentation_single_cluster() {
        let decomposed = "e\u{0301}"; // e + combining acute
        let composed = "\u{00E9}"; // precomposed é
        let (n1, s1) = normalize_and_segment(decomposed);
        let (n2, s2) = normalize_and_segment(composed);
        assert_eq!(n1, n2);
        assert_eq!(s1.len(), 1);
        assert_eq!(s2.len(), 1);
        assert_eq!(s1[0].cluster, "é");
        assert_eq!(s2[0].cluster, "é");
        assert_eq!(s1[0].width, s2[0].width);
    }

    #[test]
    fn segmentation_zwj_family_and_cjk() {
        let s = "漢😀👨‍👩‍👧‍👦a";
        let (_n, segs) = normalize_and_segment(s);
        // Expect at least 4 segments
        assert!(segs.len() >= 4);
        // Byte ranges monotonically increase and clusters concat to normalized
        let mut prev_end = 0usize;
        let mut join = String::new();
        for seg in &segs {
            assert!(seg.start == prev_end);
            assert!(seg.end >= seg.start);
            prev_end = seg.end;
            join.push_str(&seg.cluster);
        }
        // NFC of original should equal join (since we normalized)
        assert_eq!(join, s.nfc().collect::<String>());
    }

    #[test]
    fn gear_vs16_width_override_respected() {
        // Expect width adapter to apply override mapping gear+VS16 to width 1 (as in existing tests)
        let s = "a⚙️b";
        let (_n, segs) = normalize_and_segment(s);
        assert_eq!(segs.len(), 3);
        assert_eq!(segs[1].cluster, "⚙️");
        assert_eq!(segs[1].width, 1);
    }
}
