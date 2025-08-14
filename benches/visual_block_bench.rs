use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use oxidized::core::mode::{Position, Selection, SelectionType};
use std::hint::black_box;
use std::time::Duration;

fn make_lines(lines: usize, base_len: usize) -> Vec<String> {
    let mut v = Vec::with_capacity(lines);
    for i in 0..lines {
        let mut s = String::with_capacity(base_len + 8);
        for j in 0..base_len.max(1) {
            // pseudo-random ascii pattern
            let c = (((i * 31 + j * 17) % 26) as u8 + b'a') as char;
            s.push(c);
        }
        v.push(s);
    }
    v
}

fn bench_block_highlight(c: &mut Criterion) {
    let mut group = c.benchmark_group("visual_block_highlight");
    group
        .sample_size(40)
        .warm_up_time(Duration::from_millis(200));

    // (case_name, lines, line_len, block_height, block_width)
    let cases = [
        ("small", 200, 80, 4, 8),
        ("medium", 1000, 120, 12, 24),
        ("tall", 5000, 80, 64, 12),
        ("wide", 1200, 240, 8, 80),
        ("unicode_mix", 1500, 100, 16, 40),
    ];

    for (name, lines, line_len, block_h, block_w) in cases {
        let text = make_lines(lines, line_len);
        // Precompute per-line lengths to avoid counting inside the loop (mimics renderer caching)
        let lengths: Vec<usize> = text.iter().map(|s| s.chars().count()).collect();

        group.bench_with_input(BenchmarkId::new("block", name), &lines, |b, _| {
            b.iter(|| {
                // Simulate moving the cursor to create different block selections
                let mut acc: usize = 0;
                for anchor_row in (0..lines.saturating_sub(block_h.max(1))).step_by(block_h.max(1))
                {
                    let start = Position {
                        row: anchor_row,
                        col: 3,
                    };
                    let end = Position {
                        row: anchor_row + block_h.min(lines - anchor_row - 1),
                        col: 3 + block_w,
                    };
                    let sel = Selection {
                        start,
                        end,
                        selection_type: SelectionType::Block,
                    };
                    // Iterate lines within selection computing highlight spans
                    for (row, &len) in lengths.iter().enumerate().take(end.row + 1).skip(start.row)
                    {
                        let span = sel.highlight_span_for_line(row, len);
                        if let Some((l, r)) = span {
                            acc ^= r.saturating_sub(l);
                        }
                    }
                }
                black_box(acc)
            })
        });
    }

    group.finish();
}

criterion_group!(benches, bench_block_highlight);
criterion_main!(benches);
