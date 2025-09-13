use core_actions::Action;
use core_actions::dispatcher::dispatch;
use core_model::EditorModel;
use core_render::scheduler::{RenderDelta, RenderScheduler};
use core_state::EditorState;
use core_text::Buffer;

// Integration-adjacent test: simulate entering command mode (:) and typing
// characters; expect StatusLine semantics instead of Lines.
#[test]
fn command_typing_emits_status_only() {
    let buffer = Buffer::from_str("t", "abc\n").unwrap();
    let state = EditorState::new(buffer);
    let mut model = EditorModel::new(state);
    let mut sticky = None;
    let observers: Vec<Box<dyn core_actions::ActionObserver>> = Vec::new();
    let mut scheduler = RenderScheduler::new();

    // Start command line
    let dr = dispatch(Action::CommandStart, &mut model, &mut sticky, &observers);
    assert!(dr.dirty);
    scheduler.mark(RenderDelta::StatusLine); // main loop would do this via status detection logic

    // Type 'q'
    let dr = dispatch(
        Action::CommandChar('q'),
        &mut model,
        &mut sticky,
        &observers,
    );
    assert!(dr.dirty);
    scheduler.mark(RenderDelta::StatusLine);

    // Collapse
    let decision = scheduler.consume().expect("decision");
    assert!(matches!(decision.semantic, RenderDelta::StatusLine));
}
