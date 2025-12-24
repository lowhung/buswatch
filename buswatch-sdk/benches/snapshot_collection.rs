use buswatch_sdk::Instrumentor;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

/// Benchmark collect() with a single module and varying topic counts
fn bench_collect_single_module_varying_topics(c: &mut Criterion) {
    let mut group = c.benchmark_group("collect_single_module");

    for topic_count in [1, 5, 10, 50, 100].iter() {
        let instrumentor = Instrumentor::new();
        let handle = instrumentor.register("bench-module");

        // Pre-populate with topics
        for i in 0..*topic_count {
            let topic = format!("topic-{}", i);
            handle.record_read(&topic, 100);
            handle.record_write(&topic, 50);
        }

        group.bench_with_input(
            BenchmarkId::from_parameter(topic_count),
            topic_count,
            |b, _| {
                b.iter(|| {
                    black_box(instrumentor.collect());
                });
            },
        );
    }
    group.finish();
}

/// Benchmark collect() with varying module counts
fn bench_collect_varying_modules(c: &mut Criterion) {
    let mut group = c.benchmark_group("collect_varying_modules");

    for module_count in [1, 5, 10, 20, 50].iter() {
        let instrumentor = Instrumentor::new();
        let mut handles = Vec::new();

        // Create multiple modules
        for i in 0..*module_count {
            let module_name = format!("module-{}", i);
            let handle = instrumentor.register(&module_name);

            // Add some metrics to each module
            for j in 0..5 {
                let topic = format!("topic-{}", j);
                handle.record_read(&topic, 100);
                handle.record_write(&topic, 50);
            }

            handles.push(handle);
        }

        group.bench_with_input(
            BenchmarkId::from_parameter(module_count),
            module_count,
            |b, _| {
                b.iter(|| {
                    black_box(instrumentor.collect());
                });
            },
        );
    }
    group.finish();
}

/// Benchmark collect() with realistic workload (multiple modules, multiple topics)
fn bench_collect_realistic_workload(c: &mut Criterion) {
    let instrumentor = Instrumentor::new();

    // Simulate a realistic scenario: 10 modules, each with 20 topics
    for i in 0..10 {
        let module_name = format!("service-{}", i);
        let handle = instrumentor.register(&module_name);

        for j in 0..20 {
            let topic = format!("events.{}.topic-{}", i, j);
            handle.record_read(&topic, i * 1000 + j * 10);
            handle.record_write(&topic, i * 500 + j * 5);
        }
    }

    c.bench_function("collect_realistic_workload", |b| {
        b.iter(|| {
            black_box(instrumentor.collect());
        });
    });
}

/// Benchmark collect() after active recording (measures impact of concurrent updates)
fn bench_collect_with_active_recording(c: &mut Criterion) {
    let instrumentor = Instrumentor::new();
    let handle = instrumentor.register("active-module");

    // Pre-populate
    for i in 0..10 {
        let topic = format!("topic-{}", i);
        handle.record_read(&topic, 1000);
        handle.record_write(&topic, 500);
    }

    c.bench_function("collect_with_active_recording", |b| {
        b.iter(|| {
            // Simulate some concurrent updates
            handle.record_read("topic-0", 1);
            handle.record_write("topic-1", 1);

            // Collect snapshot
            black_box(instrumentor.collect());
        });
    });
}

/// Benchmark empty collect() to measure baseline overhead
fn bench_collect_empty(c: &mut Criterion) {
    let instrumentor = Instrumentor::new();

    c.bench_function("collect_empty", |b| {
        b.iter(|| {
            black_box(instrumentor.collect());
        });
    });
}

/// Benchmark collect() with only registered modules (no metrics)
fn bench_collect_modules_no_metrics(c: &mut Criterion) {
    let instrumentor = Instrumentor::new();

    // Register 10 modules but don't record any metrics
    for i in 0..10 {
        let module_name = format!("module-{}", i);
        instrumentor.register(&module_name);
    }

    c.bench_function("collect_modules_no_metrics", |b| {
        b.iter(|| {
            black_box(instrumentor.collect());
        });
    });
}

criterion_group!(
    benches,
    bench_collect_single_module_varying_topics,
    bench_collect_varying_modules,
    bench_collect_realistic_workload,
    bench_collect_with_active_recording,
    bench_collect_empty,
    bench_collect_modules_no_metrics
);
criterion_main!(benches);
