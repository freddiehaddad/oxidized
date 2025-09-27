//! core-keymap: Next-Gen Input (NGI) mapping engine.
//!
//! Design principles (see design doc Phase A Step 2):
//! - Pure and deterministic: resolution depends only on buffer + context.
//! - Layered keymaps compiled into a compressed trie for cache locality.
//! - Ambiguity surfaced by returning `None` when a strict prefix of one or more
//!   mappings matches but no terminal mapping has yet been confirmed.
//! - No side effects: logging only at TRACE for traversal steps.
//!
//! This initial scaffold purposefully limits scope to what is needed to port
//! existing Normal mode translation logic (counts, operators, motions,
//! register prefix). It does NOT yet integrate timeout handling or layering.

use smallvec::SmallVec;
use tracing::{debug, trace};

// -------------------------------------------------------------------------------------------------
// Public Symbolic Output (expanded for PendingContext composition)
// -------------------------------------------------------------------------------------------------
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MappingOutput {
    CountDigit(char),     // '1'..'9' or '0' when extending an existing count
    LeadingZeroLineStart, // solitary '0' with no prior count (Normal mode semantics)
    Operator(char),       // e.g. 'd', 'y', 'c'
    Motion(char),         // placeholder: maps to MotionKind in adapter layer
    RegisterPrefix,       // '"' awaiting register designator
    RegisterName(char), // emitted only when trie layer chooses (currently unused; kept for future multi-class tokens)
    PasteAfter,         // 'p'
    PasteBefore,        // 'P'
    Undo,               // 'u'
    Redo,               // <C-r> not represented yet (requires modifier model)
    EnterInsert,        // 'i'
    ModeToggleVisualChar, // 'v'
    Esc,                // <Esc>
    DeleteUnder,        // 'x'
    DeleteLeft,         // 'X'
    DeleteToLineEnd,    // 'D' shorthand for d$
    ChangeToLineEnd,    // 'C' shorthand for c$
    Literal(char),      // fallback literal / command char (':' etc.)
}

// -------------------------------------------------------------------------------------------------
// PendingContext: accumulates syntactic prefixes (count/operator/register) similar to Vim semantics
// -------------------------------------------------------------------------------------------------
#[derive(Debug, Default, Clone)]
pub struct PendingContext {
    pub count_prefix: Option<u32>,
    pub operator: Option<char>,
    pub post_op_count: Option<u32>,
    pub register: Option<char>,
    pub awaiting_register: bool,
}

impl PendingContext {
    pub fn reset_transient(&mut self) {
        self.count_prefix = None;
        self.operator = None;
        self.post_op_count = None;
        self.awaiting_register = false;
    }
}

// -------------------------------------------------------------------------------------------------
// ComposedAction: result after feeding a sequence of MappingOutputs through PendingContext
// -------------------------------------------------------------------------------------------------
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComposedAction {
    Motion {
        motion: char,
        count: u32,
    },
    ApplyOperator {
        op: char,
        motion: char,
        count: u32,
        register: Option<char>,
    },
    LinewiseOperator {
        op: char,
        count: u32,
        register: Option<char>,
    },
    PasteAfter {
        register: Option<char>,
    },
    PasteBefore {
        register: Option<char>,
    },
    EnterInsert,
    Undo,
    ModeToggleVisualChar,
    DeleteUnder,
    DeleteLeft,
    Literal(char),
    None, // no emission (still accumulating state)
}

/// Feed a single MappingOutput through the PendingContext, possibly producing a ComposedAction.
pub fn compose_with_context(ctx: &mut PendingContext, out: &MappingOutput) -> ComposedAction {
    match out {
        MappingOutput::Esc => {
            ctx.reset_transient();
            ComposedAction::None
        }
        MappingOutput::RegisterPrefix => {
            ctx.awaiting_register = true;
            debug!(
                target = "input.context",
                awaiting_register = true,
                "register_prefix"
            );
            ComposedAction::None
        }
        MappingOutput::RegisterName(c) => {
            if ctx.awaiting_register {
                ctx.register = Some(*c);
                ctx.awaiting_register = false;
                debug!(target="input.context", register=%c, "register_set");
            }
            ComposedAction::None
        }
        MappingOutput::CountDigit(c) => {
            if ctx.operator.is_some() {
                // post-op count (except leading zero rule handled earlier)
                let digit = (*c as u8 - b'0') as u32;
                let new_val = ctx
                    .post_op_count
                    .unwrap_or(0)
                    .saturating_mul(10)
                    .saturating_add(digit)
                    .min(999_999);
                ctx.post_op_count = Some(new_val);
                debug!(target="input.context", post_op_count=new_val, digit=%c, "post_op_count_extend");
                ComposedAction::None
            } else {
                let digit = (*c as u8 - b'0') as u32;
                let new_val = ctx
                    .count_prefix
                    .unwrap_or(0)
                    .saturating_mul(10)
                    .saturating_add(digit)
                    .min(999_999);
                ctx.count_prefix = Some(new_val);
                debug!(target="input.context", count_prefix=new_val, digit=%c, "count_prefix_extend");
                ComposedAction::None
            }
        }
        MappingOutput::LeadingZeroLineStart => {
            // Zero that is not extending a count -> immediate motion '0'
            let count = ctx.count_prefix.take().unwrap_or(1);
            debug!(
                target = "input.context",
                count,
                motion = "0",
                "leading_zero_motion"
            );
            ComposedAction::Motion { motion: '0', count }
        }
        MappingOutput::Operator(op) => {
            if let Some(prev) = ctx.operator
                && prev == *op
            {
                let prefix = ctx.count_prefix.take().unwrap_or(1);
                let post = ctx.post_op_count.take().unwrap_or(1);
                let total = prefix.saturating_mul(post).min(999_999);
                let reg = ctx.register.take();
                ctx.operator = None;
                debug!(
                    target = "input.context",
                    operator = %op,
                    count = total,
                    register = ?reg,
                    "linewise_operator_emit"
                );
                return ComposedAction::LinewiseOperator {
                    op: *op,
                    count: total,
                    register: reg,
                };
            }
            ctx.operator = Some(*op);
            ctx.post_op_count = None;
            debug!(target="input.context", operator=%op, "operator_pending");
            ComposedAction::None
        }
        MappingOutput::Motion(m) => {
            if let Some(op) = ctx.operator.take() {
                // operator + motion path
                let prefix = ctx.count_prefix.take().unwrap_or(1);
                let post = ctx.post_op_count.take().unwrap_or(1);
                let total = prefix.saturating_mul(post).min(999_999);
                let reg = ctx.register.take();
                debug!(target="input.context", op=%op, motion=%m, count=total, register=?reg, "apply_operator_motion");
                ComposedAction::ApplyOperator {
                    op,
                    motion: *m,
                    count: total,
                    register: reg,
                }
            } else {
                let count = ctx.count_prefix.take().unwrap_or(1);
                debug!(target="input.context", motion=%m, count, "motion_emit");
                ComposedAction::Motion { motion: *m, count }
            }
        }
        MappingOutput::PasteAfter => ComposedAction::PasteAfter {
            register: ctx.register.take(),
        },
        MappingOutput::PasteBefore => ComposedAction::PasteBefore {
            register: ctx.register.take(),
        },
        MappingOutput::Undo => {
            debug!(target = "input.context", "undo_emit");
            ComposedAction::Undo
        }
        MappingOutput::EnterInsert => {
            debug!(target = "input.context", "enter_insert_emit");
            ComposedAction::EnterInsert
        }
        MappingOutput::ModeToggleVisualChar => {
            debug!(target = "input.context", "visual_toggle_emit");
            ComposedAction::ModeToggleVisualChar
        }
        MappingOutput::DeleteUnder => {
            if let Some(n) = ctx.count_prefix.take() {
                debug!(
                    target = "input.context",
                    dropped_count = n,
                    "delete_under_count_dropped"
                );
            }
            debug!(target = "input.context", "delete_under_emit");
            ComposedAction::DeleteUnder
        }
        MappingOutput::DeleteLeft => {
            if let Some(n) = ctx.count_prefix.take() {
                debug!(
                    target = "input.context",
                    dropped_count = n,
                    "delete_left_count_dropped"
                );
            }
            debug!(target = "input.context", "delete_left_emit");
            ComposedAction::DeleteLeft
        }
        MappingOutput::DeleteToLineEnd => {
            if let Some(n) = ctx.count_prefix.take() {
                debug!(
                    target = "input.context",
                    dropped_count = n,
                    "delete_to_eol_count_dropped"
                );
            }
            let reg = ctx.register.take();
            debug!(target = "input.context", op = "d", motion = "$", register = ?reg, "apply_operator_shorthand_eol");
            ComposedAction::ApplyOperator {
                op: 'd',
                motion: '$',
                count: 1,
                register: reg,
            }
        }
        MappingOutput::ChangeToLineEnd => {
            if let Some(n) = ctx.count_prefix.take() {
                debug!(
                    target = "input.context",
                    dropped_count = n,
                    "change_to_eol_count_dropped"
                );
            }
            let reg = ctx.register.take();
            debug!(target = "input.context", op = "c", motion = "$", register = ?reg, "apply_operator_shorthand_eol");
            ComposedAction::ApplyOperator {
                op: 'c',
                motion: '$',
                count: 1,
                register: reg,
            }
        }
        MappingOutput::Literal(c) => {
            debug!(target="input.context", ch=%c, "literal_emit");
            ComposedAction::Literal(*c)
        }
        MappingOutput::Redo => ComposedAction::None, // not yet modeled (modifiers missing)
    }
}

// -------------------------------------------------------------------------------------------------
// Key Token Pattern (scaffold)
// -------------------------------------------------------------------------------------------------
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum KeyTokenPattern {
    Char(char),
}

impl KeyTokenPattern {
    fn matches(&self, ch: char) -> bool {
        match self {
            KeyTokenPattern::Char(c) => *c == ch,
        }
    }
}

// -------------------------------------------------------------------------------------------------
// Mapping Specification
// -------------------------------------------------------------------------------------------------
#[derive(Debug, Clone)]
pub struct MappingSpec {
    pub sequence: Vec<KeyTokenPattern>,
    pub output: MappingOutput,
}

// -------------------------------------------------------------------------------------------------
// Trie Representation
// -------------------------------------------------------------------------------------------------
#[derive(Debug, Clone)]
struct Edge {
    pat: KeyTokenPattern,
    next: usize,
}

#[derive(Debug, Clone)]
struct Node {
    terminal: Option<usize>, // index into mappings vec
    edges: SmallVec<[Edge; 4]>,
}

impl Node {
    fn new() -> Self {
        Self {
            terminal: None,
            edges: SmallVec::new(),
        }
    }
}

#[derive(Debug)]
pub struct MappingTrie {
    nodes: Vec<Node>,
    mappings: Vec<MappingSpec>,
}

impl MappingTrie {
    pub fn build(specs: Vec<MappingSpec>) -> Self {
        let mut trie = MappingTrie {
            nodes: vec![Node::new()],
            mappings: specs,
        };
        for (idx, m) in trie.mappings.iter().enumerate() {
            let mut cur = 0usize;
            for pat in &m.sequence {
                // find or create edge
                let next = if let Some(e) = trie.nodes[cur].edges.iter().find(|e| e.pat == *pat) {
                    e.next
                } else {
                    let new_idx = trie.nodes.len();
                    trie.nodes.push(Node::new());
                    trie.nodes[cur].edges.push(Edge {
                        pat: pat.clone(),
                        next: new_idx,
                    });
                    new_idx
                };
                cur = next;
            }
            if trie.nodes[cur].terminal.is_some() {
                // Conflict: later mapping overrides earlier (design choice for now); log at trace.
                trace!(
                    target = "input.map",
                    mapping_index = idx,
                    node = cur,
                    "terminal_override"
                );
            }
            trie.nodes[cur].terminal = Some(idx);
        }
        trie
    }

    pub fn resolve(&self, buffer: &[char]) -> Resolution {
        let mut node_idx = 0usize;
        let mut last_terminal: Option<(usize, usize)> = None; // (consumed, mapping index)
        for (i, ch) in buffer.iter().enumerate() {
            let mut advanced = false;
            for edge in &self.nodes[node_idx].edges {
                if edge.pat.matches(*ch) {
                    node_idx = edge.next;
                    trace!(target = "input.map", step = i, ch = %ch, node = node_idx, "advance");
                    if let Some(mi) = self.nodes[node_idx].terminal {
                        last_terminal = Some((i + 1, mi));
                    }
                    advanced = true;
                    break;
                }
            }
            if !advanced {
                break;
            }
        }
        if let Some((consumed, mi)) = last_terminal {
            Resolution::Matched {
                consumed,
                output: self.mappings[mi].output.clone(),
                ambiguous: !self.nodes[node_idx].edges.is_empty(),
            }
        } else if !buffer.is_empty() {
            // If we consumed nothing (still at root) and found no edge, fallback literal.
            if node_idx == 0 {
                Resolution::FallbackLiteral(buffer[0])
            } else if !self.nodes[node_idx].edges.is_empty() {
                Resolution::NeedMore
            } else {
                Resolution::FallbackLiteral(buffer[0])
            }
        } else {
            Resolution::NeedMore
        }
    }
}

// -------------------------------------------------------------------------------------------------
// Resolution Result
// -------------------------------------------------------------------------------------------------
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolution {
    Matched {
        consumed: usize,
        output: MappingOutput,
        ambiguous: bool,
    },
    NeedMore, // strict prefix of one or more mappings (ambiguous)
    FallbackLiteral(char),
}

// -------------------------------------------------------------------------------------------------
// Baseline Normal Mode Mapping Specs (subset) for parity scaffolding
// -------------------------------------------------------------------------------------------------
pub fn baseline_normal_specs() -> Vec<MappingSpec> {
    use KeyTokenPattern as K;
    let mut v = vec![
        MappingSpec {
            sequence: vec![K::Char('d')],
            output: MappingOutput::Operator('d'),
        },
        MappingSpec {
            sequence: vec![K::Char('y')],
            output: MappingOutput::Operator('y'),
        },
        MappingSpec {
            sequence: vec![K::Char('c')],
            output: MappingOutput::Operator('c'),
        },
        MappingSpec {
            sequence: vec![K::Char('w')],
            output: MappingOutput::Motion('w'),
        },
        MappingSpec {
            sequence: vec![K::Char('b')],
            output: MappingOutput::Motion('b'),
        },
        MappingSpec {
            sequence: vec![K::Char('h')],
            output: MappingOutput::Motion('h'),
        },
        MappingSpec {
            sequence: vec![K::Char('l')],
            output: MappingOutput::Motion('l'),
        },
        MappingSpec {
            sequence: vec![K::Char('k')],
            output: MappingOutput::Motion('k'),
        },
        MappingSpec {
            sequence: vec![K::Char('j')],
            output: MappingOutput::Motion('j'),
        },
        MappingSpec {
            sequence: vec![K::Char('p')],
            output: MappingOutput::PasteAfter,
        },
        MappingSpec {
            sequence: vec![K::Char('P')],
            output: MappingOutput::PasteBefore,
        },
        MappingSpec {
            sequence: vec![K::Char('u')],
            output: MappingOutput::Undo,
        },
        MappingSpec {
            sequence: vec![K::Char('x')],
            output: MappingOutput::DeleteUnder,
        },
        MappingSpec {
            sequence: vec![K::Char('X')],
            output: MappingOutput::DeleteLeft,
        },
        MappingSpec {
            sequence: vec![K::Char('i')],
            output: MappingOutput::EnterInsert,
        },
        MappingSpec {
            sequence: vec![K::Char('D')],
            output: MappingOutput::DeleteToLineEnd,
        },
        MappingSpec {
            sequence: vec![K::Char('C')],
            output: MappingOutput::ChangeToLineEnd,
        },
        MappingSpec {
            sequence: vec![K::Char('v')],
            output: MappingOutput::ModeToggleVisualChar,
        },
        MappingSpec {
            sequence: vec![K::Char('0')],
            output: MappingOutput::LeadingZeroLineStart,
        },
        MappingSpec {
            sequence: vec![K::Char('$')],
            output: MappingOutput::Motion('$'),
        },
        MappingSpec {
            sequence: vec![K::Char('"')],
            output: MappingOutput::RegisterPrefix,
        },
    ];
    // digits 1-9
    for d in ['1', '2', '3', '4', '5', '6', '7', '8', '9'] {
        v.push(MappingSpec {
            sequence: vec![K::Char(d)],
            output: MappingOutput::CountDigit(d),
        });
    }
    // NOTE: We intentionally do NOT add generic register name tokens to the trie yet.
    // Register capture after '"' is handled by compose layer inspecting subsequent Literal outputs.
    v
}

// -------------------------------------------------------------------------------------------------
// Tests
// -------------------------------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    fn feed(seq: &str) -> Vec<ComposedAction> {
        let specs = baseline_normal_specs();
        let trie = MappingTrie::build(specs);
        let mut ctx = PendingContext::default();
        let mut out = Vec::new();
        let chars: Vec<char> = seq.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            let slice = &chars[i..];
            match trie.resolve(slice) {
                Resolution::Matched {
                    consumed, output, ..
                } => {
                    let mut out_tok = output.clone();
                    if ctx.awaiting_register
                        && let MappingOutput::Literal(c) = out_tok
                        && c.is_ascii_alphanumeric()
                    {
                        out_tok = MappingOutput::RegisterName(c);
                    }
                    let composed = compose_with_context(&mut ctx, &out_tok);
                    if let ComposedAction::None = composed { /* continue */
                    } else {
                        out.push(composed);
                    }
                    i += consumed;
                }
                Resolution::FallbackLiteral(c) => {
                    if ctx.awaiting_register && c.is_ascii_alphanumeric() {
                        let composed =
                            compose_with_context(&mut ctx, &MappingOutput::RegisterName(c));
                        if let ComposedAction::None = composed {
                        } else {
                            out.push(composed);
                        }
                    } else {
                        let composed = compose_with_context(&mut ctx, &MappingOutput::Literal(c));
                        if let ComposedAction::None = composed {
                        } else {
                            out.push(composed);
                        }
                    }
                    i += 1;
                }
                Resolution::NeedMore => {
                    break;
                }
            }
        }
        out
    }
    #[test]
    fn single_key_match() {
        let trie = MappingTrie::build(baseline_normal_specs());
        let res = trie.resolve(&['w']);
        assert_eq!(
            res,
            Resolution::Matched {
                consumed: 1,
                output: MappingOutput::Motion('w'),
                ambiguous: false
            }
        );
    }

    #[test]
    fn need_more_for_prefix() {
        // Add an artificial mapping 'd' and 'dw' to force ambiguity.
        let mut specs = baseline_normal_specs();
        specs.push(MappingSpec {
            sequence: vec![KeyTokenPattern::Char('d'), KeyTokenPattern::Char('x')],
            output: MappingOutput::Literal('!'),
        });
        let trie = MappingTrie::build(specs);
        let res = trie.resolve(&['d']);
        assert_eq!(
            res,
            Resolution::Matched {
                consumed: 1,
                output: MappingOutput::Operator('d'),
                ambiguous: true
            }
        );
    }

    #[test]
    fn multi_key_longest_match() {
        let mut specs = baseline_normal_specs();
        specs.push(MappingSpec {
            sequence: vec![KeyTokenPattern::Char('d'), KeyTokenPattern::Char('w')],
            output: MappingOutput::Literal('#'),
        });
        let trie = MappingTrie::build(specs);
        let res = trie.resolve(&['d', 'w']);
        assert_eq!(
            res,
            Resolution::Matched {
                consumed: 2,
                output: MappingOutput::Literal('#'),
                ambiguous: false
            }
        );
    }

    #[test]
    fn fallback_literal() {
        let trie = MappingTrie::build(baseline_normal_specs());
        let res = trie.resolve(&['z']);
        assert_eq!(res, Resolution::FallbackLiteral('z'));
    }

    #[test]
    fn compose_simple_motion() {
        let acts = feed("w");
        assert_eq!(
            acts,
            vec![ComposedAction::Motion {
                motion: 'w',
                count: 1
            }]
        );
    }

    #[test]
    fn compose_count_motion() {
        let acts = feed("5w");
        assert_eq!(
            acts,
            vec![ComposedAction::Motion {
                motion: 'w',
                count: 5
            }]
        );
    }

    #[test]
    fn compose_operator_motion_dw() {
        let acts = feed("dw");
        assert_eq!(
            acts,
            vec![ComposedAction::ApplyOperator {
                op: 'd',
                motion: 'w',
                count: 1,
                register: None
            }]
        );
    }

    #[test]
    fn compose_prefix_count_operator_motion_2dw() {
        let acts = feed("2dw");
        assert_eq!(
            acts,
            vec![ComposedAction::ApplyOperator {
                op: 'd',
                motion: 'w',
                count: 2,
                register: None
            }]
        );
    }

    #[test]
    fn compose_double_operator_dd() {
        let acts = feed("dd");
        assert_eq!(
            acts,
            vec![ComposedAction::LinewiseOperator {
                op: 'd',
                count: 1,
                register: None
            }]
        );
    }

    #[test]
    fn compose_prefix_count_double_operator_3dd() {
        let acts = feed("3dd");
        assert_eq!(
            acts,
            vec![ComposedAction::LinewiseOperator {
                op: 'd',
                count: 3,
                register: None
            }]
        );
    }

    #[test]
    fn compose_post_count_double_operator_d2d() {
        let acts = feed("d2d");
        assert_eq!(
            acts,
            vec![ComposedAction::LinewiseOperator {
                op: 'd',
                count: 2,
                register: None
            }]
        );
    }

    #[test]
    fn compose_register_prefixed_double_operator() {
        let acts = feed("\"add");
        assert_eq!(
            acts,
            vec![ComposedAction::LinewiseOperator {
                op: 'd',
                count: 1,
                register: Some('a')
            }]
        );
    }

    #[test]
    fn compose_operator_post_count_d2w() {
        let acts = feed("d2w");
        assert_eq!(
            acts,
            vec![ComposedAction::ApplyOperator {
                op: 'd',
                motion: 'w',
                count: 2,
                register: None
            }]
        );
    }

    #[test]
    fn compose_multiplicative_counts_2d3w() {
        let acts = feed("2d3w");
        assert_eq!(
            acts,
            vec![ComposedAction::ApplyOperator {
                op: 'd',
                motion: 'w',
                count: 6,
                register: None
            }]
        );
    }

    #[test]
    fn compose_delete_to_line_end_shorthand_d() {
        let acts = feed("D");
        assert_eq!(
            acts,
            vec![ComposedAction::ApplyOperator {
                op: 'd',
                motion: '$',
                count: 1,
                register: None
            }]
        );
    }

    #[test]
    fn compose_change_to_line_end_shorthand_c() {
        let acts = feed("C");
        assert_eq!(
            acts,
            vec![ComposedAction::ApplyOperator {
                op: 'c',
                motion: '$',
                count: 1,
                register: None
            }]
        );
    }

    #[test]
    fn compose_register_prefix_named_yw() {
        let acts = feed("\"ayw");
        assert_eq!(
            acts,
            vec![ComposedAction::ApplyOperator {
                op: 'y',
                motion: 'w',
                count: 1,
                register: Some('a')
            }]
        );
    }

    #[test]
    fn compose_register_prefix_prefix_count_a2yw() {
        let acts = feed("\"a2yw");
        assert_eq!(
            acts,
            vec![ComposedAction::ApplyOperator {
                op: 'y',
                motion: 'w',
                count: 2,
                register: Some('a')
            }]
        );
    }
}
