use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use oxidized::core::editor::Editor;
use std::hint::black_box;
use std::time::Duration;

fn setup_editor(line_len: usize, window_width: u16, siso: usize) -> Editor {
    let mut editor = Editor::new().expect("editor new");
    // Keep benches deterministic/lightweight
    editor.set_config_setting_ephemeral("syntax", "false");
    editor.set_config_setting_ephemeral("wrap", "false");
    editor.set_config_setting_ephemeral("laststatus", "false");
    editor.set_config_setting_ephemeral("showcmd", "false");
    editor.set_config_setting_ephemeral("sidescrolloff", &siso.to_string());

    let _ = editor.create_buffer(None).expect("create buffer");
    if let Some(buf) = editor.current_buffer_mut() {
        buf.lines = vec!["a".repeat(line_len)];
        buf.cursor.row = 0;
        buf.cursor.col = 0;
    }
    if let Some(win) = editor.window_manager.current_window_mut() {
        win.width = window_width;
    }
    editor
}

fn bench_viewport_hscroll(c: &mut Criterion) {
    let mut group = c.benchmark_group("viewport_hscroll");
    group
        .sample_size(12)
        .warm_up_time(Duration::from_millis(100))
        .measurement_time(Duration::from_millis(250));

    let cases = [
        ("siso0_w10_l120", 120usize, 10u16, 0usize),
        ("siso5_w12_l2000", 2000usize, 12u16, 5usize),
        ("siso20_w16_l2000", 2000usize, 16u16, 20usize),
    ];

    for (name, line_len, width, siso) in cases {
        group.bench_with_input(BenchmarkId::new("render", name), &line_len, |b, &_ll| {
            let mut editor = setup_editor(line_len, width, siso);
            // Pre-create key handler to avoid allocs in loop
            let mut kh = oxidized::input::keymap::KeyHandler::new();
            // Cursor targets (left/mid/right)
            let left = 0usize;
            let mid = (line_len / 2).min(line_len);
            let right = line_len;
            let mut state = 0u8;
            b.iter(|| {
                // Cycle cursor position to induce h-scroll logic
                let target = match state % 3 {
                    0 => left,
                    1 => mid,
                    _ => right,
                };
                state = state.wrapping_add(1);

                if let Some(buf) = editor.current_buffer_mut() {
                    buf.cursor.col = target;
                }
                // Nudge via a no-op-ish key to exercise pipeline; 'l' may advance by 1
                let _ = kh.handle_key(
                    &mut editor,
                    KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE),
                );
                // Render triggers viewport/h-scroll update; terminal is headless-safe
                let _ = editor.render();
                black_box(());
            })
        });
    }

    group.finish();
}

criterion_group!(benches, bench_viewport_hscroll);
criterion_main!(benches);
