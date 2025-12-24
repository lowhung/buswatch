use buswatch_sdk::Instrumentor;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::sync::Arc;
use std::thread;

/// Benchmark concurrent increment throughput with varying thread counts
fn bench_concurrent_increments_varying_threads(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_increments");

    for thread_count in [1, 2, 4, 8, 16].iter() {
        group.throughput(Throughput::Elements(*thread_count as u64 * 1000));
        group.bench_with_input(
            BenchmarkId::new("threads", thread_count),
            thread_count,
            |b, &thread_count| {
                b.iter(|| {
                    let instrumentor = Arc::new(Instrumentor::new());
                    let handle = Arc::new(instrumentor.register("bench-module"));

                    let mut handles_vec = vec![];

                    for _ in 0..thread_count {
                        let handle_clone = Arc::clone(&handle);
                        let join_handle = thread::spawn(move || {
                            for _ in 0..1000 {
                                handle_clone.record_read(black_box("test-topic"), black_box(1));
                            }
                        });
                        handles_vec.push(join_handle);
                    }

                    for join_handle in handles_vec {
                        join_handle.join().unwrap();
                    }
                });
            },
        );
    }
    group.finish();
}

/// Benchmark concurrent writes to same topic (high contention)
fn bench_concurrent_writes_same_topic(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_writes_same_topic");

    for thread_count in [2, 4, 8].iter() {
        group.throughput(Throughput::Elements(*thread_count as u64 * 1000));
        group.bench_with_input(
            BenchmarkId::from_parameter(thread_count),
            thread_count,
            |b, &thread_count| {
                b.iter(|| {
                    let instrumentor = Arc::new(Instrumentor::new());
                    let handle = Arc::new(instrumentor.register("bench-module"));

                    let mut handles_vec = vec![];

                    for _ in 0..thread_count {
                        let handle_clone = Arc::clone(&handle);
                        let join_handle = thread::spawn(move || {
                            for _ in 0..1000 {
                                handle_clone.record_write(black_box("shared-topic"), black_box(1));
                            }
                        });
                        handles_vec.push(join_handle);
                    }

                    for join_handle in handles_vec {
                        join_handle.join().unwrap();
                    }
                });
            },
        );
    }
    group.finish();
}

/// Benchmark concurrent reads to different topics (low contention)
fn bench_concurrent_reads_different_topics(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_reads_different_topics");

    for thread_count in [2, 4, 8].iter() {
        group.throughput(Throughput::Elements(*thread_count as u64 * 1000));
        group.bench_with_input(
            BenchmarkId::from_parameter(thread_count),
            thread_count,
            |b, &thread_count| {
                b.iter(|| {
                    let instrumentor = Arc::new(Instrumentor::new());
                    let handle = Arc::new(instrumentor.register("bench-module"));

                    let mut handles_vec = vec![];

                    for thread_id in 0..thread_count {
                        let handle_clone = Arc::clone(&handle);
                        let join_handle = thread::spawn(move || {
                            let topic = format!("topic-{}", thread_id);
                            for _ in 0..1000 {
                                handle_clone.record_read(black_box(&topic), black_box(1));
                            }
                        });
                        handles_vec.push(join_handle);
                    }

                    for join_handle in handles_vec {
                        join_handle.join().unwrap();
                    }
                });
            },
        );
    }
    group.finish();
}

/// Benchmark concurrent mixed operations (reads and writes)
fn bench_concurrent_mixed_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_mixed_operations");

    for thread_count in [2, 4, 8].iter() {
        group.throughput(Throughput::Elements(*thread_count as u64 * 2000));
        group.bench_with_input(
            BenchmarkId::from_parameter(thread_count),
            thread_count,
            |b, &thread_count| {
                b.iter(|| {
                    let instrumentor = Arc::new(Instrumentor::new());
                    let handle = Arc::new(instrumentor.register("bench-module"));

                    let mut handles_vec = vec![];

                    for thread_id in 0..thread_count {
                        let handle_clone = Arc::clone(&handle);
                        let join_handle = thread::spawn(move || {
                            let input_topic = format!("input-{}", thread_id);
                            let output_topic = format!("output-{}", thread_id);

                            for _ in 0..1000 {
                                handle_clone.record_read(black_box(&input_topic), black_box(1));
                                handle_clone.record_write(black_box(&output_topic), black_box(1));
                            }
                        });
                        handles_vec.push(join_handle);
                    }

                    for join_handle in handles_vec {
                        join_handle.join().unwrap();
                    }
                });
            },
        );
    }
    group.finish();
}

/// Benchmark concurrent access from multiple modules
fn bench_concurrent_multiple_modules(c: &mut Criterion) {
    c.bench_function("concurrent_multiple_modules", |b| {
        b.iter(|| {
            let instrumentor = Arc::new(Instrumentor::new());

            let mut handles_vec = vec![];

            // 8 threads, each with their own module
            for thread_id in 0..8 {
                let instrumentor_clone = Arc::clone(&instrumentor);
                let join_handle = thread::spawn(move || {
                    let module_name = format!("module-{}", thread_id);
                    let handle = instrumentor_clone.register(&module_name);

                    for i in 0..500 {
                        let topic = format!("topic-{}", i % 10);
                        handle.record_read(black_box(&topic), black_box(1));
                        handle.record_write(black_box(&topic), black_box(1));
                    }
                });
                handles_vec.push(join_handle);
            }

            for join_handle in handles_vec {
                join_handle.join().unwrap();
            }
        });
    });
}

/// Benchmark concurrent increments with collect() calls
fn bench_concurrent_with_collect(c: &mut Criterion) {
    c.bench_function("concurrent_with_collect", |b| {
        b.iter(|| {
            let instrumentor = Arc::new(Instrumentor::new());
            let handle = Arc::new(instrumentor.register("bench-module"));

            let mut handles_vec = vec![];

            // Spawn 4 writer threads
            for thread_id in 0..4 {
                let handle_clone = Arc::clone(&handle);
                let join_handle = thread::spawn(move || {
                    let topic = format!("topic-{}", thread_id);
                    for _ in 0..500 {
                        handle_clone.record_write(black_box(&topic), black_box(1));
                    }
                });
                handles_vec.push(join_handle);
            }

            // Spawn 1 collector thread
            let instrumentor_clone = Arc::clone(&instrumentor);
            let collector_handle = thread::spawn(move || {
                for _ in 0..10 {
                    black_box(instrumentor_clone.collect());
                }
            });
            handles_vec.push(collector_handle);

            for join_handle in handles_vec {
                join_handle.join().unwrap();
            }
        });
    });
}

criterion_group!(
    benches,
    bench_concurrent_increments_varying_threads,
    bench_concurrent_writes_same_topic,
    bench_concurrent_reads_different_topics,
    bench_concurrent_mixed_operations,
    bench_concurrent_multiple_modules,
    bench_concurrent_with_collect
);
criterion_main!(benches);
