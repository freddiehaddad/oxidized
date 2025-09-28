mod common;
use common::*;

use core_events::{KeyCode, KeyEvent, KeyModifiers};
use core_model::EditorModel;
use core_text::Buffer;

fn kc(c: char) -> KeyEvent {
    KeyEvent {
        code: KeyCode::Char(c),
        mods: KeyModifiers::empty(),
    }
}

// Helper to dispatch a key sequence in Normal mode.
fn feed(model: &mut EditorModel, seq: &str) {
    let mut sticky = None;
    for ch in seq.chars() {
        let ev = kc(ch);
        if let Some(act) =
            translate_key(model.state().mode, model.state().command_line.buffer(), &ev)
        {
            core_actions::dispatcher::dispatch(act, model, &mut sticky, &[]);
        }
    }
}

#[test]
fn explicit_numbered_register_paste_1p() {
    reset_translator();
    // Build a buffer with three words to exercise deletes.
    let buf = Buffer::from_str("t", "alpha beta gamma delta\n").unwrap();
    let state = core_state::EditorState::new(buf);
    let mut model = EditorModel::new(state);

    // Sequence: delete first word (dw), then delete next (dw), then yank next (yw)
    // This yields numbered ring (newest first):
    // 0: yank payload, 1: second delete payload, 2: first delete payload.
    feed(&mut model, "dw"); // delete 'alpha '
    feed(&mut model, "dw"); // delete 'beta '
    feed(&mut model, "yw"); // yank 'gamma'
    // Explicit paste from ring[1] should yield second delete payload ('beta ')
    feed(&mut model, "0\"1p");
    let line0 = model.state().active_buffer().line(0).unwrap();
    assert!(
        line0.contains("beta"),
        "expected ring[1] payload 'beta ' inserted, got {}",
        line0
    );
}

#[test]
fn explicit_numbered_register_zero_paste_latest() {
    reset_translator();
    let buf = Buffer::from_str("t", "one two three four\n").unwrap();
    let state = core_state::EditorState::new(buf);
    let mut model = EditorModel::new(state);

    // Populate ring: delete first two words then yank third.
    feed(&mut model, "dw"); // 'one '
    feed(&mut model, "dw"); // 'two '
    feed(&mut model, "yw"); // yank 'three'
    // ring[0] = "three" (yank), ring[1] = "two ", ring[2] = "one "
    feed(&mut model, "0\"0p"); // paste latest (yank payload)
    let line0 = model.state().active_buffer().line(0).unwrap();
    assert!(
        line0.contains("three"),
        "expected paste of latest yank 'three', got {}",
        line0
    );
}
