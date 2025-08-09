use criterion::{BatchSize, BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use oxidized::features::search::SearchEngine;

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

    let cases = vec![
        ("short", 200, 60, "abc", 31),
        ("medium", 1_000, 80, "needle", 53),
        ("unicode", 800, 72, "βγ", 47),
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
