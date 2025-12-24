use buswatch_sdk::Instrumentor;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

/// Benchmark record_read latency (hot path)
fn bench_record_read(c: &mut Criterion) {
    let instrumentor = Instrumentor::new();
    let handle = instrumentor.register("bench-module");

    c.bench_function("record_read", |b| {
        b.iter(|| {
            handle.record_read(black_box("test-topic"), black_box(1));
        });
    });
}

/// Benchmark record_write latency (hot path)
fn bench_record_write(c: &mut Criterion) {
    let instrumentor = Instrumentor::new();
    let handle = instrumentor.register("bench-module");

    c.bench_function("record_write", |b| {
        b.iter(|| {
            handle.record_write(black_box("test-topic"), black_box(1));
        });
    });
}

/// Benchmark record_read with varying counts
fn bench_record_read_varying_counts(c: &mut Criterion) {
    let mut group = c.benchmark_group("record_read_varying_counts");
    let instrumentor = Instrumentor::new();
    let handle = instrumentor.register("bench-module");

    for count in [1u64, 10, 100, 1000].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(count), count, |b, &count| {
            b.iter(|| {
                handle.record_read(black_box("test-topic"), black_box(count));
            });
        });
    }
    group.finish();
}

/// Benchmark record_write with varying counts
fn bench_record_write_varying_counts(c: &mut Criterion) {
    let mut group = c.benchmark_group("record_write_varying_counts");
    let instrumentor = Instrumentor::new();
    let handle = instrumentor.register("bench-module");

    for count in [1u64, 10, 100, 1000].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(count), count, |b, &count| {
            b.iter(|| {
                handle.record_write(black_box("test-topic"), black_box(count));
            });
        });
    }
    group.finish();
}

/// Benchmark mixed read/write operations
fn bench_mixed_operations(c: &mut Criterion) {
    let instrumentor = Instrumentor::new();
    let handle = instrumentor.register("bench-module");

    c.bench_function("mixed_read_write", |b| {
        b.iter(|| {
            handle.record_read(black_box("input-topic"), black_box(1));
            handle.record_write(black_box("output-topic"), black_box(1));
        });
    });
}

/// Benchmark operations across multiple topics
fn bench_multiple_topics(c: &mut Criterion) {
    let mut group = c.benchmark_group("multiple_topics");
    let instrumentor = Instrumentor::new();
    let handle = instrumentor.register("bench-module");

    for topic_count in [1, 5, 10, 20].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(topic_count),
            topic_count,
            |b, &topic_count| {
                b.iter(|| {
                    for i in 0..topic_count {
                        let topic = format!("topic-{}", i);
                        handle.record_read(black_box(&topic), black_box(1));
                    }
                });
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_record_read,
    bench_record_write,
    bench_record_read_varying_counts,
    bench_record_write_varying_counts,
    bench_mixed_operations,
    bench_multiple_topics
);
criterion_main!(benches);
