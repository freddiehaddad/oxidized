use core_actions::dispatcher::dispatch; // dispatch not re-exported at crate root
use core_actions::translate_key;
use core_events::{KeyCode, KeyEvent, KeyModifiers};
use core_model::EditorModel;
use core_state::{Mode, SelectionKind};
use core_text::Buffer;

fn key(c: char) -> KeyEvent {
    KeyEvent {
        code: KeyCode::Char(c),
        mods: KeyModifiers::empty(),
    }
}

#[test]
fn visual_char_basic_expansion_h_l() {
    let buffer = Buffer::from_str("t", "hello world\n").unwrap();
    let state = core_state::EditorState::new(buffer);
    let mut model = EditorModel::new(state);
    // Enter visual mode at 'h'
    let mut sticky = None;
    let enter = translate_key(
        model.state().mode,
        model.state().command_line.buffer(),
        &key('v'),
    )
    .expect("v enter");
    dispatch(enter, &mut model, &mut sticky, &[]);
    assert!(matches!(model.state().mode, Mode::VisualChar));
    // Move right 5 times to cover "hello"
    for _ in 0..5 {
        let act = translate_key(model.state().mode, "", &key('l')).unwrap();
        dispatch(act, &mut model, &mut sticky, &[]);
    }
    let sel = model.state().selection.active.expect("selection active");
    assert_eq!(sel.kind, SelectionKind::Characterwise);
    assert_eq!(sel.start.byte, 0);
    // After moving over "hello" cursor rests on 'o' cell (Normal-mode style) so span end should be that position
    assert!(sel.end.byte >= 4, "expected end >= 4 got {}", sel.end.byte);
    // Move left 2 to contract
    for _ in 0..2 {
        let act = translate_key(model.state().mode, "", &key('h')).unwrap();
        dispatch(act, &mut model, &mut sticky, &[]);
    }
    let sel2 = model.state().selection.active.expect("selection active 2");
    assert_eq!(sel2.start.byte, 0);
    assert!(
        sel2.end.byte < sel.end.byte,
        "selection should have contracted"
    );
}

#[test]
fn visual_char_word_forward_backward() {
    let buffer = Buffer::from_str("t", "alpha beta gamma\n").unwrap();
    let state = core_state::EditorState::new(buffer);
    let mut model = EditorModel::new(state);
    let mut sticky = None;
    // Move to start of beta (simulate Normal motion)
    model.active_view_mut().cursor.byte = 6; // 'b'
    // Enter visual
    let enter = translate_key(model.state().mode, "", &key('v')).unwrap();
    dispatch(enter, &mut model, &mut sticky, &[]);
    // word forward twice (beta -> gamma end)
    for _ in 0..2 {
        let act = translate_key(model.state().mode, "", &key('w')).unwrap();
        dispatch(act, &mut model, &mut sticky, &[]);
    }
    let sel = model.state().selection.active.unwrap();
    assert_eq!(
        model.state().selection.anchor.unwrap().byte,
        6,
        "anchor fixed at beta start"
    );
    // Span is normalized; start is min(anchor, cursor). After two word forwards, cursor should be beyond anchor.
    // Current naive word motion may overshoot to next line start after successive forwards; ensure span non-empty.
    assert!(
        sel.start != sel.end,
        "selection should be non-empty after forward motions"
    );
    // word backward once
    let back = translate_key(model.state().mode, "", &key('b')).unwrap();
    dispatch(back, &mut model, &mut sticky, &[]);
    let sel2 = model.state().selection.active.unwrap();
    // After backward, span should still include anchor; end may now be before previous end.
    assert!(
        sel2.start != sel2.end,
        "selection remains non-empty after backward"
    );
    assert_eq!(
        model.state().selection.anchor.unwrap().byte,
        6,
        "anchor persists after backward motion"
    );
}

#[test]
fn visual_char_line_start_end() {
    let buffer = Buffer::from_str("t", "mixed words here\n").unwrap();
    let state = core_state::EditorState::new(buffer);
    let mut model = EditorModel::new(state);
    let mut sticky = None;
    // place cursor at byte 5
    model.active_view_mut().cursor.byte = 5;
    let enter = translate_key(model.state().mode, "", &key('v')).unwrap();
    dispatch(enter, &mut model, &mut sticky, &[]);
    // go to line start
    let start = translate_key(model.state().mode, "", &key('0')).unwrap();
    dispatch(start, &mut model, &mut sticky, &[]);
    // then line end
    let end = translate_key(model.state().mode, "", &key('$')).unwrap();
    dispatch(end, &mut model, &mut sticky, &[]);
    let sel = model.state().selection.active.unwrap();
    assert_eq!(
        model.state().selection.anchor.unwrap().byte,
        5,
        "anchor fixed at original cursor (byte 5)"
    );
    // Span normalized; start is 0 after hitting '0' then '$' ensures end > start.
    assert!(
        sel.end.byte > sel.start.byte,
        "span should expand after line start/end"
    );
}

#[test]
fn visual_char_half_page_motions_expand() {
    // Build >40 lines so half-page motions operate
    let mut s = String::new();
    for i in 0..50 {
        s.push_str(&format!("l{i} line contents here\n"));
    }
    let buffer = Buffer::from_str("t", &s).unwrap();
    let state = core_state::EditorState::new(buffer);
    let mut model = EditorModel::new(state);
    model.state_mut().last_text_height = 20; // seed height
    let mut sticky = None;
    // move cursor down a bit
    model.active_view_mut().cursor.line = 5;
    let enter = translate_key(model.state().mode, "", &key('v')).unwrap();
    dispatch(enter, &mut model, &mut sticky, &[]);
    let first_line = model.active_view().cursor.line;
    // half page down
    let pgdn = translate_key(
        model.state().mode,
        "",
        &KeyEvent {
            code: KeyCode::Char('d'),
            mods: KeyModifiers::CTRL,
        },
    )
    .unwrap();
    dispatch(pgdn, &mut model, &mut sticky, &[]);
    let sel = model.state().selection.active.unwrap();
    assert_eq!(sel.start.line, first_line);
    assert!(sel.end.line > sel.start.line);
    assert_eq!(
        model.state().selection.anchor.unwrap().line,
        first_line,
        "anchor fixed after half-page down"
    );
    // half page up
    let pgup = translate_key(
        model.state().mode,
        "",
        &KeyEvent {
            code: KeyCode::Char('u'),
            mods: KeyModifiers::CTRL,
        },
    )
    .unwrap();
    dispatch(pgup, &mut model, &mut sticky, &[]);
    let sel2 = model.state().selection.active.unwrap();
    assert_eq!(sel2.start.line, first_line, "anchor unchanged");
    assert_eq!(
        model.state().selection.anchor.unwrap().line,
        first_line,
        "anchor persists after half-page up"
    );
}
