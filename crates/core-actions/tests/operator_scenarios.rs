mod common;
use common::*;

// Step 6.4 operator scenario & invariant harness.
// Provides reusable helpers to exercise operator + motion combinations
// and assert buffer restoration + structural repaint contracts.

use core_actions::{Action, dispatcher::DispatchResult, dispatcher::dispatch};
use core_events::{KeyCode, KeyEvent, KeyModifiers};
use core_model::EditorModel;
use core_text::Buffer;

/// Scenario step: either feed raw key(s) or invoke direct action.
#[derive(Debug, Clone)]
pub enum Step<'a> {
    Keys(&'a str),  // sequence of literal keys like "d2w" or "dj"
    Action(Action), // direct action (Undo, Redo, etc.)
}

#[derive(Debug, Default, Clone)]
pub struct ScenarioExpect {
    pub final_text: Option<&'static str>,
    pub unnamed_register_contains: Option<&'static str>,
    pub buffer_replaced: Option<bool>,
}

/// Runs a scenario: starting text, list of steps, optional expected end state + invariants.
pub fn run_scenario(initial: &str, steps: &[Step<'_>], expect: ScenarioExpect) -> DispatchResult {
    let buffer = Buffer::from_str("t", initial).unwrap();
    let state = core_state::EditorState::new(buffer);
    let mut model = EditorModel::new(state);
    let mut sticky = None;
    let mut last_res = DispatchResult::clean();

    for step in steps {
        match step {
            Step::Keys(seq) => {
                for ch in seq.chars() {
                    // TODO: Extend for <Esc>, <C-d>, etc. when needed.
                    let evt = KeyEvent {
                        code: KeyCode::Char(ch),
                        mods: KeyModifiers::empty(),
                    };
                    if let Some(act) = translate_key(
                        model.state().mode,
                        model.state().command_line.buffer(),
                        &evt,
                    ) {
                        last_res = dispatch(act, &mut model, &mut sticky, &[]);
                    }
                }
            }
            Step::Action(act) => {
                last_res = dispatch(act.clone(), &mut model, &mut sticky, &[]);
            }
        }
    }

    if let Some(text) = expect.final_text {
        let mut collected = String::new();
        let b = model.state().active_buffer();
        for i in 0..b.line_count() {
            collected.push_str(&b.line(i).unwrap());
        }
        assert_eq!(collected, text, "final buffer text mismatch");
    }

    if let Some(substr) = expect.unnamed_register_contains {
        assert!(
            model.state().registers.unnamed.contains(substr),
            "register invariant failed: expected substring '{substr}' in unnamed register, got '{}'",
            model.state().registers.unnamed
        );
    }

    if let Some(br) = expect.buffer_replaced {
        assert_eq!(last_res.buffer_replaced, br, "buffer_replaced mismatch");
    }

    last_res
}

#[cfg(test)]
mod tests {
    use super::Step::*;
    use super::*;
    use core_actions::Action;

    // dj u : multi-line delete then undo restores original text; both structural
    #[test]
    fn scenario_dj_undo() {
        let initial = "l1\nl2\nl3\n"; // trailing newline
        let res = run_scenario(
            initial,
            &[Keys("dj"), Action(Action::Undo { count: 1 })],
            ScenarioExpect {
                final_text: Some(initial),
                buffer_replaced: Some(true),
                ..Default::default()
            },
        );
        assert!(res.buffer_replaced);
    }

    // 2dw u : single line intra-line delete (non-structural) then undo (non-structural) restores text
    #[test]
    fn scenario_2dw_undo() {
        let initial = "one two three four\n";
        run_scenario(
            initial,
            &[
                Keys("2dw"), // expects: delete two words starting at cursor ("one ", "two ")
                Action(Action::Undo { count: 1 }),
            ],
            ScenarioExpect {
                final_text: Some(initial),
                buffer_replaced: Some(false),
                ..Default::default()
            },
        );
    }

    // dj dj u u : stacked multi-line deletions and undos
    #[test]
    fn scenario_dj_dj_undo_undo() {
        let initial = "a1\na2\na3\na4\n";
        run_scenario(
            initial,
            &[
                Keys("dj"),
                Keys("dj"),
                Action(Action::Undo { count: 1 }),
                Action(Action::Undo { count: 1 }),
            ],
            ScenarioExpect {
                final_text: Some(initial),
                buffer_replaced: Some(true),
                ..Default::default()
            },
        );
    }
}
