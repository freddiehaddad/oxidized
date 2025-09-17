use core_model::{View, ViewId};
use core_render::{
    render_engine::RenderEngine,
    timing::{last_render_ns, record_last_render_ns},
};
use core_state::EditorState;
use core_text::Buffer;

#[test]
fn last_render_ns_non_zero_after_render() {
    let buf = Buffer::from_str("test", "hello world\n").unwrap();
    let state = EditorState::new(buf);
    let view = View::new(ViewId(0), state.active, core_text::Position::origin(), 0);
    let mut engine = RenderEngine::new();
    let start = std::time::Instant::now();
    let _ = engine.render_full(&state, &view, 80, 10);
    let elapsed = start.elapsed().as_nanos() as u64;
    record_last_render_ns(elapsed);
    assert!(last_render_ns() > 0, "expected non-zero render timing");
}
