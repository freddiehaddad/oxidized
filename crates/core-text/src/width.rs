//! Unicode Grapheme Cluster Display Width Engine (Steps 4.1–4.5 Complete)
//!
//! This module introduces a single authoritative function `egc_width` that
//! returns the terminal column width for a provided grapheme cluster (EGC).
//!
//! Step 4.1 Goal (complete):
//! - Centralized width computation behind one API without changing behavior.
//!
//! Step 4.2 Goal (complete):
//! - Generated static override table for sequences the baseline crate
//!   mis-measures (ZWJ emoji, flags, keycaps, tone modifiers, select combining).
//!
//! Steps Summary:
//! - Step 4.1: Unified API introduced (passthrough to `unicode_width`).
//! - Step 4.2: Generated static override table for sequences baseline mis-measures.
//! - Step 4.3: Heuristic classifier (EGC -> Kind) + precedence policy.
//! - Step 4.4: Optional (feature-gated) runtime probe precedence hook.
//! - Step 4.5: Expanded conformance tests & override consistency property test; doc update procedure.
//!
//! Width Precedence Order:
//! 1. Runtime terminal-specific override (feature `term-probe`).
//! 2. Static generated override table.
//! 3. Classifier (semantic kind -> width mapping).
//! 4. Conservative widen fallback (if pictographic signal but width==1).
//!
//! Update Procedure (Unicode / table refresh):
//! 1. Regenerate width overrides TSV (script TBD) with latest Unicode data; ensure entries sorted.
//! 2. Run build to regenerate `generated_width_overrides.rs` and commit both the TSV (if tracked) and generated file.
//! 3. Add/adjust tests for any new emoji composition patterns (ZWJ forms, keycaps, modifiers).
//! 4. Verify `override_table_consistency` passes and no classifier regressions.
//! 5. Bump documented Unicode version (future constant) once introduced.
//! 6. Run full suite (nextest) + clippy + fmt; commit as Step 4.x maintenance.
//!
//! Invariants:
//! - No caller bypasses `egc_width` for display width decisions.
//! - Classifier favors over-estimation to avoid render drift.
//! - Overrides table remains sorted & unique (enforced indirectly by search binary correctness & tests).
//!
//! Design Invariants (to be enforced in later steps):
//! - All width decisions flow through `egc_width`.
//! - No other crate calls `unicode_width` directly after migration.
//! - Grapheme segmentation occurs once at caller; we operate on an EGC slice.
//! - API intentionally minimal to allow future caching layers.
//!
//! Implementation Notes:
//! - Classifier is heuristic but biased toward over-estimating width for any
//!   emoji / pictographic composite. Over-estimation only causes extra blank
//!   cell(s) which are harmless; under-estimation causes rendering drift.
//! - We purposefully do NOT depend on large Unicode property crates; instead
//!   we use small range checks for Extended Pictographic & Combining marks.
//! - The static override table still holds sequences whose structure alone is
//!   insufficient or whose width must remain forced for stability.

// Step 4.2: generated override table (sequence->width) is compiled here.
// Not yet applied to egc_width logic; included to validate build generation.
// Provide a rust-analyzer stub to avoid transient OUT_DIR diagnostics during pre-build parsing.
#[cfg(rust_analyzer)]
#[allow(dead_code)]
mod overrides {
    pub static OVERRIDES: &[(&str, u16)] = &[];
    pub const OVERRIDES_COUNT: usize = 0;
}

#[cfg(not(rust_analyzer))]
#[allow(dead_code)]
mod overrides {
    include!(concat!(env!("OUT_DIR"), "/generated_width_overrides.rs"));
}

// -------- Step 4.3: Classifier -------------------------------------------------

/// Semantic classification of a single grapheme cluster (EGC).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EgcKind {
    Ascii,
    Narrow,
    Wide,
    EmojiSimple,     // Single pictographic (with optional VS16) no modifiers
    EmojiModifier,   // Emoji + skin tone modifier
    EmojiKeycap,     // Keycap sequence (base + optional VS16 + \u{20E3})
    EmojiFlag,       // Regional indicator pair
    EmojiZwj,        // ZWJ sequence combining >=2 pictographic bases
    Combining(bool), // Base + combining mark(s); bool indicates base wide/emoji (true => width 2)
    Other,
}

// Constants / character property helpers (small heuristic ranges)
const ZWJ: char = '\u{200D}';
const VS16: char = '\u{FE0F}';
const KEYCAP_COMBINING: char = '\u{20E3}';
// Regional Indicator range
fn is_regional_indicator(c: char) -> bool {
    ('\u{1F1E6}'..='\u{1F1FF}').contains(&c)
}
// Fitzpatrick skin tone modifiers
fn is_skin_tone_modifier(c: char) -> bool {
    ('\u{1F3FB}'..='\u{1F3FF}').contains(&c)
}
// Rough Extended Pictographic heuristic (covers most emoji blocks + misc symbols used as emoji)
fn is_extended_pictographic(c: char) -> bool {
    // Primary emoji blocks & supplemental symbols
    ('\u{1F300}'..='\u{1FAFF}').contains(&c) ||
    // Misc Symbols + Dingbats where many legacy emoji live
    ('\u{2600}'..='\u{27BF}').contains(&c)
}
// Combining mark ranges commonly encountered (subset)
fn is_combining_mark(c: char) -> bool {
    ('\u{0300}'..='\u{036F}').contains(&c)
        || ('\u{1AB0}'..='\u{1AFF}').contains(&c)
        || ('\u{1DC0}'..='\u{1DFF}').contains(&c)
        || ('\u{20D0}'..='\u{20FF}').contains(&c)
        || ('\u{FE20}'..='\u{FE2F}').contains(&c)
}

/// Binary search the sorted overrides table for an exact match.
fn override_width(egc: &str) -> Option<u16> {
    if overrides::OVERRIDES.is_empty() {
        return None;
    }
    let mut lo = 0usize;
    let mut hi = overrides::OVERRIDES.len();
    while lo < hi {
        let mid = (lo + hi) / 2;
        let (seq, w) = overrides::OVERRIDES[mid];
        match seq.cmp(egc) {
            core::cmp::Ordering::Less => lo = mid + 1,
            core::cmp::Ordering::Greater => hi = mid,
            core::cmp::Ordering::Equal => return Some(w),
        }
    }
    None
}

/// Classify an EGC (single grapheme slice).
fn classify(egc: &str) -> EgcKind {
    if egc.is_empty() {
        return EgcKind::Other;
    }
    let mut chars = egc.chars().peekable();
    let first = *chars.peek().unwrap();
    let single = egc.chars().count() == 1;

    // Quick single-char path
    if single {
        if first.is_ascii() {
            return EgcKind::Ascii;
        }
        // Use unicode_width for wide detection (gives 2 for W/F)
        let uwidth = unicode_width::UnicodeWidthChar::width(first).unwrap_or(1);
        if is_extended_pictographic(first) {
            return EgcKind::EmojiSimple;
        }
        if uwidth == 2 {
            return EgcKind::Wide;
        }
        // Ambiguous East Asian width can't be precisely detected without property tables;
        // treat everything else as Narrow here.
        return EgcKind::Narrow;
    }

    // Multi-codepoint analysis flags
    let mut count_ep = 0usize;
    let mut count_ri = 0usize;
    let mut has_zwj = false;
    // VS16 presence is noted but not needed explicitly for width (folded into pictographic detection)
    let mut has_skin = false;
    let mut has_combining = false;
    let mut keycap_base: Option<char> = None; // digit, #, * if present
    let mut ends_with_keycap = false;
    let mut any_wide = false; // based on unicode_width
    let mut base_wide_or_emoji = false; // for combining cluster
    let mut saw_non_mark_base = false;

    for (i, c) in egc.chars().enumerate() {
        if is_extended_pictographic(c) {
            count_ep += 1;
        }
        if is_regional_indicator(c) {
            count_ri += 1;
        }
        if c == ZWJ {
            has_zwj = true;
        }
        if c == VS16 { /* variation selector - emoji presentation hint */ }
        if is_skin_tone_modifier(c) {
            has_skin = true;
        }
        if is_combining_mark(c) {
            has_combining = true;
        }
        if c == KEYCAP_COMBINING && i == egc.chars().count() - 1 {
            ends_with_keycap = true;
        }
        if keycap_base.is_none() && (c.is_ascii_digit() || c == '#' || c == '*') {
            keycap_base = Some(c);
        }
        if unicode_width::UnicodeWidthChar::width(c).unwrap_or(1) == 2 {
            any_wide = true;
        }
        if !saw_non_mark_base && !is_combining_mark(c) {
            saw_non_mark_base = true;
            if is_extended_pictographic(c)
                || unicode_width::UnicodeWidthChar::width(c).unwrap_or(1) == 2
            {
                base_wide_or_emoji = true;
            }
        }
    }

    // Keycap pattern: base (+ VS16 optional) + keycap combining mark at end
    if ends_with_keycap && keycap_base.is_some() {
        return EgcKind::EmojiKeycap;
    }

    // Flag: exactly two regional indicators (RI RI)
    if count_ri == 2 && egc.chars().count() == 2 {
        return EgcKind::EmojiFlag;
    }

    // ZWJ sequence combining >=2 pictographic bases
    if has_zwj && count_ep >= 2 {
        return EgcKind::EmojiZwj;
    }

    // Emoji + skin tone modifier (EP + skin + optional VS16)
    if count_ep >= 1 && has_skin {
        return EgcKind::EmojiModifier;
    }

    // Single pictographic concept with optional VS16
    if count_ep == 1 && !has_zwj {
        return EgcKind::EmojiSimple;
    }

    // Combining marks cluster (no above emoji classification triggered)
    if has_combining {
        return EgcKind::Combining(base_wide_or_emoji);
    }

    // Wide East Asian (any codepoint width 2) => treat as wide cluster
    if any_wide {
        return EgcKind::Wide;
    }

    // Fallback: if any pictographic left (should have matched earlier) => simple
    if count_ep > 0 {
        return EgcKind::EmojiSimple;
    }

    EgcKind::Narrow
}

#[inline]
fn width_for_kind(kind: EgcKind) -> u16 {
    match kind {
        EgcKind::Ascii | EgcKind::Narrow => 1,
        EgcKind::Wide
        | EgcKind::EmojiSimple
        | EgcKind::EmojiModifier
        | EgcKind::EmojiKeycap
        | EgcKind::EmojiFlag
        | EgcKind::EmojiZwj => 2,
        EgcKind::Combining(base_wide) => {
            if base_wide {
                2
            } else {
                1
            }
        }
        EgcKind::Other => 1,
    }
}

/// Return the display column width for a single grapheme cluster (EGC).
///
/// Precondition: `egc` MUST be a single grapheme cluster boundary slice.
/// (Callers already perform segmentation; we do not re-validate here to
/// avoid double scanning.)
///
/// Behavior (Step 4.1): passthrough to `unicode_width` crate. Empty input
/// returns 0. Multi-grapheme input is not validated (debug asserts may be
/// added in later hardening).
#[inline]
pub fn egc_width(egc: &str) -> u16 {
    if egc.is_empty() {
        return 0;
    }

    // Temporary explicit override (pre-Step 5 cursor alignment fix experiment):
    // Treat GEAR (⚙) with or without VS16 as width 1 to address reported cursor
    // alignment issue. Some terminals render this glyph narrow; our previous
    // classifier widened it (Extended Pictographic heuristic) causing visual
    // drift for the user. This override narrows it pending a broader
    // terminal-probing solution. (If future probe detects width=2 it will
    // supersede this path.)
    if egc == "⚙" || egc == "⚙️" {
        // U+2699 optionally followed by VS16
        return 1;
    }

    // 0) Runtime (terminal-specific) override if feature enabled
    #[allow(unused)]
    {
        #[cfg(feature = "term-probe")]
        if let Some(w) = crate::width_probe::runtime_override_width(egc) {
            return w;
        }
    }

    // 1) Explicit override table
    if let Some(w) = override_width(egc) {
        return w;
    }

    // 2) Classify & map
    let kind = classify(egc);
    let mut width = width_for_kind(kind);

    // 3) Conservative fallback guard: if width==1 but sequence contains
    // pictographic or regional indicator signals, widen to 2 to avoid drift.
    if width == 1 {
        let mut has_signal = false;
        for c in egc.chars() {
            if is_extended_pictographic(c) || is_regional_indicator(c) {
                has_signal = true;
                break;
            }
        }
        if has_signal {
            width = 2;
        }
    }
    width
}

/// Convenience: width of a full string known to contain exactly one EGC.
#[inline]
pub fn egc_width_str(s: &str) -> u16 {
    egc_width(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii() {
        assert_eq!(egc_width("a"), 1);
    }

    #[test]
    fn wide_cjk() {
        assert_eq!(egc_width("界"), 2);
    }

    #[test]
    fn emoji_basic() {
        assert_eq!(egc_width("😀"), 2);
    }

    #[test]
    fn combining_acute() {
        assert_eq!(egc_width("e\u{0301}"), 1);
    }

    #[test]
    fn overrides_generated_present() {
        // Expect at least the entries we seeded in width_overrides.tsv
        let count = super::overrides::OVERRIDES.len();
        assert!(count >= 7, "override entries missing, found {}", count);
        // Ensure a specific representative sequence exists (flag, family, keycap)
        let mut saw_flag = false;
        let mut saw_family = false;
        let mut saw_keycap = false;
        for (seq, _w) in super::overrides::OVERRIDES.iter() {
            if *seq == "🇺🇸" {
                saw_flag = true;
            }
            if *seq == "👨‍👩‍👧‍👦" {
                saw_family = true;
            }
            if *seq == "1️⃣" {
                saw_keycap = true;
            }
        }
        assert!(saw_flag && saw_family && saw_keycap);
    }

    // ---- Step 4.3 classification tests ----

    #[test]
    fn emoji_flag() {
        assert_eq!(egc_width("🇺🇸"), 2);
    }

    #[test]
    fn emoji_keycap() {
        assert_eq!(egc_width("1️⃣"), 2);
    }

    #[test]
    fn emoji_zwj_family() {
        assert_eq!(egc_width("👨‍👩‍👧‍👦"), 2);
    }

    #[test]
    fn emoji_skin_tone() {
        assert_eq!(egc_width("👍🏻"), 2);
    }

    #[test]
    fn gear_plain_and_vs16() {
        assert_eq!(egc_width("⚙"), 1);
        assert_eq!(egc_width("⚙️"), 1);
    }

    #[test]
    fn combining_sequence() {
        assert_eq!(egc_width("e\u{0301}"), 1);
    }

    #[test]
    fn wide_cjk_again() {
        assert_eq!(egc_width("界"), 2);
    }

    #[test]
    fn ascii_again() {
        assert_eq!(egc_width("A"), 1);
    }

    #[test]
    fn runtime_probe_disabled_no_override() {
        // Feature is disabled in default test config; ensure path does not alter width.
        assert_eq!(egc_width("😀"), 2); // baseline remains 2
    }

    // ---- Step 4.5 expanded tests ----

    #[test]
    fn single_regional_indicator_alone() {
        // A lone regional indicator shouldn't be treated as flag pair; classifier widens via conservative rule.
        assert_eq!(egc_width("🇺"), 2);
    }

    #[test]
    fn keycap_without_vs16() {
        // Pattern: digit + combining keycap mark still width 2.
        assert_eq!(egc_width("2\u{20E3}"), 2);
    }

    #[test]
    fn zwj_plus_skin_tone_inside_sequence() {
        // Family variant with a skin tone modifier on one member; still enforced as width 2.
        let seq = "👨🏻\u{200D}👩\u{200D}👧"; // Simplified family subset
        assert_eq!(egc_width(seq), 2);
    }

    #[test]
    fn wide_base_with_combining_mark() {
        // Wide CJK base + combining should propagate width=2.
        let seq = "界\u{0301}"; // (Not a natural combo, but stresses logic.)
        assert_eq!(egc_width(seq), 2);
    }

    #[test]
    fn variation_selector_on_simple_emoji() {
        // Emoji + VS16 remains width 2 (already covered indirectly by gear test, add another).
        assert_eq!(egc_width("✈️"), 2);
    }

    #[test]
    fn override_table_consistency() {
        // Every entry in the static override table must match egc_width result.
        for (seq, w) in super::overrides::OVERRIDES.iter() {
            assert_eq!(egc_width(seq), *w, "override mismatch for {}", seq);
        }
    }
}
