use core_model::EditorModel;
use core_render::style::{StyleAttr, StyleLayer, StyleSpan};
use core_text::Buffer;

// Helper to manually rebuild style layer (mirrors engine logic) for assertions.
fn build_layer(model: &EditorModel) -> StyleLayer {
    let view = model.active_view();
    let state = model.state();
    let mut layer = StyleLayer::new();
    if let Some(line) = state.active_buffer().line(view.cursor.line) {
        let content_trim: &str = if line.ends_with(['\n', '\r']) {
            &line[..line.len() - 1]
        } else {
            &line
        };
        let vis_col = core_text::grapheme::visual_col(content_trim, view.cursor.byte) as u16;
        let next = core_text::grapheme::next_boundary(content_trim, view.cursor.byte);
        let cluster = &content_trim[view.cursor.byte..next];
        let w = core_text::grapheme::cluster_width(cluster).max(1) as u16;
        layer.push(StyleSpan {
            line: view.cursor.line,
            start_col: vis_col,
            end_col: vis_col + w,
            attr: StyleAttr::InvertCursor,
        });
    }
    layer
}

fn mk_model(text: &str) -> EditorModel {
    let buf = Buffer::from_str("t", text).unwrap();
    EditorModel::new(core_state::EditorState::new(buf))
}

#[test]
fn cursor_span_single_width_ascii() {
    let mut model = mk_model("abc\n");
    model.active_view_mut().cursor.line = 0;
    model.active_view_mut().cursor.byte = 1; // 'b'
    let layer = build_layer(&model);
    let span = layer.cursor_span().unwrap();
    assert_eq!(span.start_col, 1);
    assert_eq!(span.end_col, 2);
}

#[test]
fn cursor_span_wide_emoji() {
    let mut model = mk_model("a😀b\n");
    let line = model.state().active_buffer().line(0).unwrap();
    let emoji_byte = line.char_indices().find(|(_, c)| *c == '😀').unwrap().0;
    model.active_view_mut().cursor.line = 0;
    model.active_view_mut().cursor.byte = emoji_byte;
    let layer = build_layer(&model);
    let span = layer.cursor_span().unwrap();
    // 'a' is width 1 so emoji starts at col 1 and width should be 2
    assert_eq!(span.start_col, 1);
    assert_eq!(span.end_col, 3);
}

#[test]
fn cursor_span_combining_cluster() {
    let mut model = mk_model("e\u{0301}x\n");
    model.active_view_mut().cursor.line = 0;
    model.active_view_mut().cursor.byte = 0; // start of combining sequence
    let layer = build_layer(&model);
    let span = layer.cursor_span().unwrap();
    assert_eq!(span.start_col, 0);
    assert_eq!(span.end_col, 1); // combining sequence width 1
}
