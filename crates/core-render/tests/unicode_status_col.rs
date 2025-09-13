use core_model::EditorModel;
use core_render::render_engine::RenderEngine;
use core_state::EditorState;
use core_text::Buffer;

fn mk_model(text: &str) -> EditorModel {
    let st = EditorState::new(Buffer::from_str("test", text).unwrap());
    EditorModel::new(st)
}

// Helper to extract status line string from a full frame (relies on same construction path
// used in production full renders). This is a narrow test harness; if the frame height or
// status line structure changes future adjustments may be needed.
fn status_line_str(model: &EditorModel) -> String {
    let view = model.active_view().clone();
    let buf = model.state().active_buffer();
    let line_content = buf.line(view.cursor.line).unwrap_or_default();
    let content_trim: &str = if line_content.ends_with(['\n', '\r']) {
        &line_content[..line_content.len() - 1]
    } else {
        line_content.as_str()
    };
    let col = core_text::grapheme::visual_col(content_trim, view.cursor.byte);

    core_render::status::build_status(&core_render::status::StatusContext {
        mode: model.state().mode,
        line: view.cursor.line,
        col,
        command_active: model.state().command_line.is_active(),
        command_buffer: model.state().command_line.buffer(),
        file_name: model.state().file_name.as_deref(),
        dirty: model.state().dirty,
    })
}

#[test]
fn cursor_only_unicode_column_correct_single_emoji() {
    // Leading emoji (grapheme width 2). After moving cursor right one grapheme, reported column should be 3 (1-based)
    // so 0-based visual column is 2. We assert status line reflects this after a cursor-only partial render.
    let mut model = mk_model("😀 text\n");
    let mut eng = RenderEngine::new();
    let view0 = model.active_view().clone();
    let layout = core_model::Layout::single(40, 5);
    let status_line = core_render::render_engine::build_status_line(model.state(), &view0);
    eng.render_full(model.state(), &view0, &layout, 40, 5, &status_line)
        .unwrap();
    {
        let v = model.active_view_mut();
        // Move byte index by one grapheme boundary (emoji cluster length)
        let next = core_text::grapheme::next_boundary("😀 text", 0);
        v.cursor.byte = next; // same line
    }
    let moved_view = model.active_view().clone();
    let layout = core_model::Layout::single(40, 5);
    let status_line_moved =
        core_render::render_engine::build_status_line(model.state(), &moved_view);
    eng.render_cursor_only(
        model.state(),
        &moved_view,
        &layout,
        40,
        5,
        &status_line_moved,
    )
    .unwrap();
    // Rebuild status (full frame path) to compare textual column number. Should be Col 3 (1-based) if width 2.
    let status = status_line_str(&model);
    assert!(
        status.contains("Col 3"),
        "status line did not reflect visual column 3: {status}"
    );
}

#[test]
fn cursor_only_unicode_column_cjk_wide() {
    // CJK wide char '漢' has width 2.
    let mut model = mk_model("漢A\n");
    let mut eng = RenderEngine::new();
    let view0 = model.active_view().clone();
    let layout = core_model::Layout::single(40, 5);
    let status_line = core_render::render_engine::build_status_line(model.state(), &view0);
    eng.render_full(model.state(), &view0, &layout, 40, 5, &status_line)
        .unwrap();
    {
        let v = model.active_view_mut();
        let next = core_text::grapheme::next_boundary("漢A", 0);
        v.cursor.byte = next;
    }
    let moved_view = model.active_view().clone();
    let layout = core_model::Layout::single(40, 5);
    let status_line_moved =
        core_render::render_engine::build_status_line(model.state(), &moved_view);
    eng.render_cursor_only(
        model.state(),
        &moved_view,
        &layout,
        40,
        5,
        &status_line_moved,
    )
    .unwrap();
    let status = status_line_str(&model);
    assert!(
        status.contains("Col 3"),
        "CJK wide column incorrect: {status}"
    );
}

#[test]
fn cursor_only_unicode_column_combining_mark() {
    // "e01" (e + combining acute) should advance visual column by 1 grapheme.
    let mut model = mk_model("e\u{0301}x\n");
    let mut eng = RenderEngine::new();
    let view0 = model.active_view().clone();
    let layout = core_model::Layout::single(40, 5);
    let status_line = core_render::render_engine::build_status_line(model.state(), &view0);
    eng.render_full(model.state(), &view0, &layout, 40, 5, &status_line)
        .unwrap();
    {
        let v = model.active_view_mut();
        let next = core_text::grapheme::next_boundary("e\u{0301}x", 0);
        v.cursor.byte = next;
    }
    let moved_view = model.active_view().clone();
    let layout = core_model::Layout::single(40, 5);
    let status_line_moved =
        core_render::render_engine::build_status_line(model.state(), &moved_view);
    eng.render_cursor_only(
        model.state(),
        &moved_view,
        &layout,
        40,
        5,
        &status_line_moved,
    )
    .unwrap();
    let status = status_line_str(&model);
    // After moving over first grapheme (e + combining), column should be 2 (1-based Col 2)
    assert!(
        status.contains("Col 2"),
        "Combining mark column incorrect: {status}"
    );
}

#[test]
fn cursor_only_unicode_column_ascii() {
    let mut model = mk_model("abc\n");
    let mut eng = RenderEngine::new();
    let view0 = model.active_view().clone();
    let layout = core_model::Layout::single(40, 5);
    let status_line = core_render::render_engine::build_status_line(model.state(), &view0);
    eng.render_full(model.state(), &view0, &layout, 40, 5, &status_line)
        .unwrap();
    {
        let v = model.active_view_mut();
        let next = core_text::grapheme::next_boundary("abc", 0);
        v.cursor.byte = next;
    }
    let moved_view = model.active_view().clone();
    let layout = core_model::Layout::single(40, 5);
    let status_line_moved =
        core_render::render_engine::build_status_line(model.state(), &moved_view);
    eng.render_cursor_only(
        model.state(),
        &moved_view,
        &layout,
        40,
        5,
        &status_line_moved,
    )
    .unwrap();
    let status = status_line_str(&model);
    assert!(status.contains("Col 2"), "ASCII column incorrect: {status}");
}
