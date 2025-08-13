use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use oxidized::ui::UI;
use std::hint::black_box;
use std::time::Duration;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

fn make_line(kind: &str, len: usize) -> String {
    match kind {
        "ascii" => "a".repeat(len),
        "cjk" => "漢".repeat(len / 2).chars().take(len).collect(),
        "emoji" => "🙂".repeat(len / 2).chars().take(len).collect(),
        "combining" => {
            // repeating "e01" (é) combining sequence
            let unit = "e\u{0301}";
            unit.repeat(len)
        }
        _ => (0..len)
            .map(|i| char::from_u32('a' as u32 + (i % 26) as u32).unwrap())
            .collect(),
    }
}

fn bench_wrap_next_end_byte(c: &mut Criterion) {
    let mut group = c.benchmark_group("wrap_next_end_byte");
    group
        .sample_size(15)
        .warm_up_time(Duration::from_millis(100))
        .measurement_time(Duration::from_millis(250));

    let kinds = ["ascii", "cjk", "emoji", "combining"];
    let widths = [10usize, 40, 120];
    // Trim the largest length to keep runtime reasonable while preserving scaling
    let lens = [80usize, 2000];

    for kind in kinds.iter() {
        for &width in &widths {
            for &len in &lens {
                let s = make_line(kind, len);
                // Try from fewer start positions (0 and mid) to reduce total cases
                let mut starts = vec![0usize, len / 2];
                starts.sort_unstable();
                starts.dedup();
                for &start_chars in &starts {
                    // convert start to byte boundary by walking char_indices
                    let mut start_byte = 0usize;
                    if start_chars > 0 {
                        for (i, (b, _)) in s.char_indices().enumerate() {
                            if i == start_chars {
                                start_byte = b;
                                break;
                            }
                        }
                    }
                    for &word_break in &[false, true] {
                        group.bench_with_input(
                            BenchmarkId::new(
                                format!(
                                    "{}-w{}-l{}-{}-{}",
                                    kind,
                                    width,
                                    len,
                                    start_chars,
                                    if word_break { "wb" } else { "nw" }
                                ),
                                width,
                            ),
                            &width,
                            |b, &w| {
                                let ui = UI::new();
                                b.iter(|| {
                                    let (end, seg_count) = ui.wrap_next_end_byte(
                                        black_box(&s),
                                        black_box(start_byte),
                                        black_box(w),
                                        black_box(word_break),
                                    );
                                    black_box((end, seg_count))
                                })
                            },
                        );
                    }
                }
            }
        }
    }

    group.finish();
}

fn bench_unicode_width_and_graphemes(c: &mut Criterion) {
    let mut group = c.benchmark_group("unicode_width_graphemes");
    group
        .sample_size(15)
        .warm_up_time(Duration::from_millis(100))
        .measurement_time(Duration::from_millis(250));

    let kinds = ["ascii", "cjk", "emoji", "combining"];
    // Trim largest case for speed; still covers small and moderate lengths
    let lens = [80usize, 2000];

    for kind in kinds.iter() {
        for &len in &lens {
            let s = make_line(kind, len);
            group.bench_with_input(
                BenchmarkId::new(format!("{}-width-{}", kind, len), len),
                &len,
                |b, _| {
                    b.iter(|| {
                        let w = UnicodeWidthStr::width(black_box(s.as_str()));
                        black_box(w)
                    })
                },
            );
            group.bench_with_input(
                BenchmarkId::new(format!("{}-graphemes-{}", kind, len), len),
                &len,
                |b, _| {
                    b.iter(|| {
                        let mut cnt = 0usize;
                        for g in black_box(s.as_str()).graphemes(true) {
                            cnt += g.len();
                        }
                        black_box(cnt)
                    })
                },
            );
        }
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_wrap_next_end_byte,
    bench_unicode_width_and_graphemes
);
criterion_main!(benches);
