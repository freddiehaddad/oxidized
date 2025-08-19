use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use oxidized::config::EditorConfig;
use oxidized::core::editor::EditorRenderState;
use oxidized::core::window::WindowManager;
use oxidized::ui::UI;
use std::time::Duration;

fn make_editor_state(_total_lines: usize, width: u16) -> EditorRenderState {
    let config = EditorConfig::load();
    let window_manager = WindowManager::new(width, 24);
    EditorRenderState {
        mode: oxidized::core::mode::Mode::Normal,
        current_buffer: None,
        all_buffers: Default::default(),
        command_line: String::new(),
        status_message: String::new(),
        buffer_count: 1,
        current_buffer_id: None,
        current_window_id: None,
        window_manager,
        syntax_highlights: Default::default(),
        command_completion: oxidized::features::completion::CommandCompletionBuilder::new().build(),
        config,
        filetype: None,
        macro_recording: None,
        search_total: 0,
        search_index: None,
        markdown_preview_buffer_id: None,
    }
}

fn bench_gutter_and_status(c: &mut Criterion) {
    let mut group = c.benchmark_group("gutter_status");
    group
        .sample_size(20)
        .warm_up_time(Duration::from_millis(80))
        .measurement_time(Duration::from_millis(220));

    let total_lines_cases = [1usize, 1_000, 100_000];
    let widths = [40u16, 80u16, 120u16];

    for &lines in &total_lines_cases {
        for &w in &widths {
            group.bench_with_input(BenchmarkId::new("gutter", lines), &w, |b, &_width| {
                let ui = UI::new();
                b.iter(|| ui.compute_gutter_width(lines))
            });
            group.bench_with_input(BenchmarkId::new("status", lines), &w, |b, &width| {
                let ui = UI::new();
                let ers = make_editor_state(lines, width);
                b.iter(|| ui.compute_status_line_text(&ers, width))
            });
        }
    }

    group.finish();
}

criterion_group!(benches, bench_gutter_and_status);
criterion_main!(benches);
