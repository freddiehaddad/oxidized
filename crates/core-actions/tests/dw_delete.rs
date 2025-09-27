use core_actions::dispatcher::dispatch; // dispatch is not re-exported at crate root
use core_actions::{Action, MotionKind, OperatorKind, translate_key};
use core_events::{KeyCode, KeyEvent, KeyModifiers};
use core_model::EditorModel;
use core_text::Buffer;

fn key(c: char) -> KeyEvent {
    KeyEvent {
        code: KeyCode::Char(c),
        mods: KeyModifiers::empty(),
    }
}

// Helper: perform 'dw' at current cursor.
fn apply_dw(model: &mut EditorModel) {
    let mut sticky = None;
    // 'd'
    translate_key(
        model.state().mode,
        model.state().command_line.buffer(),
        &key('d'),
    );
    // 'w'
    let act = translate_key(
        model.state().mode,
        model.state().command_line.buffer(),
        &key('w'),
    )
    .expect("dw motion");
    if let Action::ApplyOperator {
        op, motion, count, ..
    } = act
    {
        assert!(matches!(op, OperatorKind::Delete));
        assert!(matches!(motion, MotionKind::WordForward));
        assert_eq!(count, 1);
    } else {
        panic!("expected ApplyOperator");
    }
    dispatch(act, model, &mut sticky, &[]);
}

#[test]
fn dw_deletes_word_plus_space_ascii() {
    let content = "*Nothing is stable. Everything can change. Comet\n";
    let buffer = Buffer::from_str("t", content).unwrap();
    let state = core_state::EditorState::new(buffer);
    let mut model = EditorModel::new(state);
    // Position cursor at start of 'Everything'
    let line0 = model.state().active_buffer().line(0).unwrap();
    let off = line0.find("Everything").expect("word present");
    model.active_view_mut().cursor.byte = off;
    apply_dw(&mut model);
    assert_eq!(model.state().registers.unnamed, "Everything ");
    let new_line = model.state().active_buffer().line(0).unwrap();
    assert!(!new_line.contains("Everything "));
}

#[test]
fn dw_deletes_word_plus_space_after_multibyte_prefix() {
    // Include multi-byte graphemes before target word: 🚀 (4 bytes) café (é multibyte) Ωmega (Ω multi-byte).
    let content = "🚀 café Ωmega Everything followed by text\n";
    let buffer = Buffer::from_str("t", content).unwrap();
    let state = core_state::EditorState::new(buffer);
    let mut model = EditorModel::new(state);
    let line0 = model.state().active_buffer().line(0).unwrap();
    let off = line0.find("Everything").expect("word present");
    model.active_view_mut().cursor.byte = off;
    apply_dw(&mut model);
    assert_eq!(model.state().registers.unnamed, "Everything ");
    let new_line = model.state().active_buffer().line(0).unwrap();
    assert!(!new_line.contains("Everything "));
}

// --- Phase 5 Step 6 explicit register prefix tests (added here for proximity to operator tests) ---

fn translate_seq(model: &mut EditorModel, seq: &str) -> Option<Action> {
    let mut last = None;
    for c in seq.chars() {
        last = translate_key(
            model.state().mode,
            model.state().command_line.buffer(),
            &key(c),
        );
    }
    last
}

#[test]
fn explicit_named_register_yank() {
    let buffer = Buffer::from_str("t", "alpha beta gamma\n").unwrap();
    let state = core_state::EditorState::new(buffer);
    let mut model = EditorModel::new(state);
    // "ayw
    let act = translate_seq(&mut model, "\"ayw").expect("apply operator");
    if let Action::ApplyOperator {
        op,
        motion,
        count,
        register,
    } = act
    {
        assert!(matches!(op, OperatorKind::Yank));
        assert!(matches!(motion, MotionKind::WordForward));
        assert_eq!(count, 1);
        assert_eq!(register, Some('a'));
    } else {
        panic!("expected ApplyOperator with register")
    }
    let mut sticky = None;
    dispatch(act, &mut model, &mut sticky, &[]);
    assert!(
        model
            .state()
            .registers
            .get_named('a')
            .unwrap()
            .starts_with("alpha")
    );
    // Unnamed should mirror
    assert!(model.state().registers.unnamed.starts_with("alpha"));
}

#[test]
fn explicit_named_register_append_uppercase() {
    let buffer = Buffer::from_str("t", "one two three four\n").unwrap();
    let state = core_state::EditorState::new(buffer);
    let mut model = EditorModel::new(state);
    // First yank into 'a'
    let act1 = translate_seq(&mut model, "\"ayw").unwrap();
    let mut sticky = None;
    dispatch(act1, &mut model, &mut sticky, &[]);
    let initial = model.state().registers.get_named('a').unwrap().to_string();
    assert!(initial.starts_with("one"));
    // Now append using "A
    let act2 = translate_seq(&mut model, "\"Ayw").unwrap();
    dispatch(act2, &mut model, &mut sticky, &[]);
    let appended = model.state().registers.get_named('a').unwrap().to_string();
    assert!(appended.starts_with(&initial));
    assert!(appended.len() >= initial.len());
}

#[test]
fn explicit_register_with_counts_2yw_equivalent_a2yw() {
    let buffer = Buffer::from_str("t", "one two three four\n").unwrap();
    let state = core_state::EditorState::new(buffer);
    let mut model = EditorModel::new(state);
    // a2yw (register a, prefix count 2, operator y, motion w) -> captures two words
    let act = translate_seq(&mut model, "\"a2yw").unwrap();
    if let Action::ApplyOperator {
        op,
        motion,
        count,
        register,
    } = act
    {
        assert!(matches!(op, OperatorKind::Yank));
        assert!(matches!(motion, MotionKind::WordForward));
        assert_eq!(count, 2, "prefix count should propagate");
        assert_eq!(register, Some('a'));
    } else {
        panic!("expected ApplyOperator with register")
    }
}

#[test]
fn explicit_register_paste_uses_named() {
    let buffer = Buffer::from_str("t", "foo bar baz\n").unwrap();
    let state = core_state::EditorState::new(buffer);
    let mut model = EditorModel::new(state);
    // Yank a word into register b
    let op = translate_seq(&mut model, "\"byw").unwrap();
    let mut sticky = None;
    dispatch(op, &mut model, &mut sticky, &[]);
    let named = model.state().registers.get_named('b').unwrap().to_string();
    // Move cursor to end of line start (simulate position) then paste with explicit register b: "bp
    let paste = translate_seq(&mut model, "\"bp").expect("paste action");
    if let Action::PasteAfter { register, count } = paste {
        assert_eq!(count, 1);
        assert_eq!(register, Some('b'));
    } else {
        panic!("expected PasteAfter struct")
    }
    dispatch(paste, &mut model, &mut sticky, &[]);
    // Named register should remain intact
    assert_eq!(model.state().registers.get_named('b').unwrap(), named);
}
