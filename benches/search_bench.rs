use criterion::{BatchSize, BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use oxidized::features::search::SearchEngine;
use std::time::Duration;

fn make_text(lines: usize, line_len: usize, pattern: &str, freq: usize) -> Vec<String> {
    let mut text = Vec::with_capacity(lines);
    for i in 0..lines {
        let mut line = String::with_capacity(line_len + 16);
        // deterministic content with some unicode to exercise UTF-8 path
        line.push_str("αβγ ");
        for j in 0..line_len.max(1) {
            let c = (((i + j) % 26) as u8 + b'a') as char;
            line.push(c);
            if j % freq == 0 {
                line.push_str(pattern);
            }
        }
        text.push(line);
    }
    text
}

fn bench_search(c: &mut Criterion) {
    let mut group = c.benchmark_group("search_engine");
    // Light tuning for stability while keeping runs fast
    group
        .sample_size(30)
        .warm_up_time(Duration::from_millis(200));

    let cases = vec![
        ("short", 500, 80, "abc", 37),
        ("medium", 2_000, 96, "needle", 59),
        ("unicode", 1_200, 88, "βγ", 43),
        ("single_ascii", 3_000, 80, "a", 13),
        ("single_unicode", 3_000, 80, "β", 17),
    ];

    for (name, lines, len, pat, freq) in cases {
        // Prepare shared text once per case
        let text = make_text(lines, len, pat, freq);

        // case-insensitive
        group.bench_with_input(
            BenchmarkId::new(format!("{}-ci", name), pat),
            &pat.to_string(),
            |b, p| {
                b.iter_batched(
                    || {
                        let mut se = SearchEngine::new();
                        se.set_case_sensitive(false);
                        se.set_use_regex(false);
                        se
                    },
                    |mut se| {
                        let results = se.search(black_box(p), black_box(&text));
                        black_box(results.len())
                    },
                    BatchSize::SmallInput,
                )
            },
        );

        // case-sensitive
        group.bench_with_input(
            BenchmarkId::new(format!("{}-cs", name), pat),
            &pat.to_string(),
            |b, p| {
                b.iter_batched(
                    || {
                        let mut se = SearchEngine::new();
                        se.set_case_sensitive(true);
                        se.set_use_regex(false);
                        se
                    },
                    |mut se| {
                        let results = se.search(black_box(p), black_box(&text));
                        black_box(results.len())
                    },
                    BatchSize::SmallInput,
                )
            },
        );

        // regex
        group.bench_with_input(
            BenchmarkId::new(format!("{}-re", name), pat),
            &pat.to_string(),
            |b, p| {
                b.iter_batched(
                    || {
                        let mut se = SearchEngine::new();
                        se.set_use_regex(true);
                        se
                    },
                    |mut se| {
                        let results = se.search(black_box(p), black_box(&text));
                        black_box(results.len())
                    },
                    BatchSize::SmallInput,
                )
            },
        );
    }

    group.finish();
}

criterion_group!(benches, bench_search);
criterion_main!(benches);
