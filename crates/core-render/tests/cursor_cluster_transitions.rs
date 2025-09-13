use core_model::Layout;
use core_render::render_engine::{RenderEngine, build_status_line};
use core_state::{EditorState, Mode};
use core_text::{Buffer, Position, grapheme};

// Build state, view, engine similar to other cursor tests.
fn setup(
    buffer_content: &str,
    last_text_height: usize,
) -> (EditorState, core_model::View, RenderEngine, u16, u16) {
    let buf = Buffer::from_str("test", buffer_content).unwrap();
    let mut state = EditorState::new(buf);
    state.mode = Mode::Normal;
    state.last_text_height = last_text_height; // pretend viewport text rows
    let view = core_model::View {
        id: core_model::ViewId(0),
        buffer_index: 0,
        cursor: Position::new(0, 0),
        viewport_first_line: 0,
    };
    (
        state,
        view,
        RenderEngine::new(),
        80,
        (last_text_height as u16) + 1,
    ) // +1 for status
}

// Derive cursor span using same logic as style_layer tests (local helper).
fn compute_cursor_span(state: &EditorState, view: &core_model::View) -> (u16, u16) {
    let buf = state.active_buffer();
    let line = buf.line(view.cursor.line).unwrap();
    let content_trim: &str = if line.ends_with(['\n', '\r']) {
        &line[..line.len() - 1]
    } else {
        &line
    };
    let vis_col = grapheme::visual_col(content_trim, view.cursor.byte) as u16;
    let next = grapheme::next_boundary(content_trim, view.cursor.byte);
    let cluster = &content_trim[view.cursor.byte..next];
    let w = grapheme::cluster_width(cluster).max(1) as u16;
    (vis_col, vis_col + w)
}

// Helper: perform a full render then cursor-only render after moving cursor; return span.
fn move_and_cursor_only(
    engine: &mut RenderEngine,
    state: &EditorState,
    view: &core_model::View,
    w: u16,
    h: u16,
) -> (u16, u16) {
    let layout = Layout::single(w, h);
    let status = build_status_line(state, view);
    engine
        .render_cursor_only(state, view, &layout, w, h, &status)
        .expect("cursor-only render");
    compute_cursor_span(state, view)
}

#[test]
fn cursor_transition_ascii_wide_ascii() {
    // Sequence: a😀b so moving from 'a' (width1) to emoji (width2) and to 'b' (width1)
    let (state, mut view, mut engine, w, h) = setup("a😀b\n", 5);
    let layout = Layout::single(w, h);
    let status0 = build_status_line(&state, &view);
    engine
        .render_full(&state, &view, &layout, w, h, &status0)
        .unwrap();

    let line = state.active_buffer().line(0).unwrap();
    let emoji_byte = line.char_indices().find(|(_, c)| *c == '😀').unwrap().0;

    view.cursor.byte = emoji_byte; // onto emoji
    let (start, end) = move_and_cursor_only(&mut engine, &state, &view, w, h);
    assert_eq!(start, 1, "emoji should start after 'a'");
    assert_eq!(end - start, 2, "emoji width 2");

    // Move to 'b'
    view.cursor.byte = line.char_indices().find(|(_, c)| *c == 'b').unwrap().0;
    let (b_start, b_end) = move_and_cursor_only(&mut engine, &state, &view, w, h);
    assert_eq!(b_start, 3, "b should start after emoji occupying cols 1-2");
    assert_eq!(b_end - b_start, 1);
}

#[test]
fn cursor_transition_plain_vs16() {
    // Gear (⚙) plain vs variation selector 16 sequence (U+2699 U+FE0F)
    let (state, mut view, mut engine, w, h) = setup("⚙⚙️x\n", 5);
    let layout = Layout::single(w, h);
    let status0 = build_status_line(&state, &view);
    engine
        .render_full(&state, &view, &layout, w, h, &status0)
        .unwrap();
    let line = state.active_buffer().line(0).unwrap();

    // Plain gear at start
    view.cursor.byte = 0;
    let (plain_start, plain_end) = move_and_cursor_only(&mut engine, &state, &view, w, h);
    let plain_width = plain_end - plain_start;
    assert!((1..=2).contains(&plain_width));

    // Second gear+VS16 cluster (start at next grapheme boundary)
    let second_start = grapheme::next_boundary(&line, 0);
    view.cursor.byte = second_start;
    let (vs_start, vs_end) = move_and_cursor_only(&mut engine, &state, &view, w, h);
    let vs_width = vs_end - vs_start;
    assert_eq!(
        vs_width, plain_width,
        "variation selector cluster width parity"
    );

    // Move to trailing 'x'
    let after_vs16 = grapheme::next_boundary(&line, second_start);
    view.cursor.byte = after_vs16; // 'x'
    let (x_start, x_end) = move_and_cursor_only(&mut engine, &state, &view, w, h);
    assert_eq!(x_end - x_start, 1);
}

#[test]
fn cursor_transition_combining_to_wide() {
    // Combining sequence (e + acute) then wide emoji then ascii
    let (state, mut view, mut engine, w, h) = setup("e\u{0301}😀z\n", 5);
    let layout = Layout::single(w, h);
    let status0 = build_status_line(&state, &view);
    engine
        .render_full(&state, &view, &layout, w, h, &status0)
        .unwrap();
    let line = state.active_buffer().line(0).unwrap();

    view.cursor.byte = 0; // combining seq
    let (comb_start, comb_end) = move_and_cursor_only(&mut engine, &state, &view, w, h);
    assert_eq!(comb_end - comb_start, 1, "combining seq width 1");

    let emoji_start = grapheme::next_boundary(&line, 0); // after combining cluster
    view.cursor.byte = emoji_start;
    let (em_start, em_end) = move_and_cursor_only(&mut engine, &state, &view, w, h);
    assert_eq!(em_end - em_start, 2, "emoji width 2");

    let after_emoji = grapheme::next_boundary(&line, emoji_start);
    view.cursor.byte = after_emoji; // 'z'
    let (z_start, z_end) = move_and_cursor_only(&mut engine, &state, &view, w, h);
    assert_eq!(z_end - z_start, 1);
}
