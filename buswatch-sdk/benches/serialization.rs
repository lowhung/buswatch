use buswatch_sdk::Instrumentor;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

/// Benchmark JSON serialization of snapshots with varying sizes
fn bench_json_serialization_varying_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("json_serialization");

    // Test different snapshot sizes
    let configs = vec![
        ("small", 1, 5),     // 1 module, 5 topics
        ("medium", 5, 20),   // 5 modules, 20 topics each
        ("large", 10, 50),   // 10 modules, 50 topics each
        ("xlarge", 20, 100), // 20 modules, 100 topics each
    ];

    for (name, module_count, topic_count) in configs {
        let instrumentor = Instrumentor::new();

        // Create modules and populate with metrics
        for i in 0..module_count {
            let module_name = format!("module-{}", i);
            let handle = instrumentor.register(&module_name);

            for j in 0..topic_count {
                let topic = format!("topic-{}", j);
                handle.record_read(&topic, i * 1000 + j * 10);
                handle.record_write(&topic, i * 500 + j * 5);
            }
        }

        let snapshot = instrumentor.collect();

        // Measure serialized size for throughput calculation
        let json = serde_json::to_string(&snapshot).unwrap();
        group.throughput(Throughput::Bytes(json.len() as u64));

        group.bench_with_input(
            BenchmarkId::from_parameter(name),
            &snapshot,
            |b, snapshot| {
                b.iter(|| {
                    black_box(serde_json::to_string(snapshot).unwrap());
                });
            },
        );
    }
    group.finish();
}

/// Benchmark pretty-printed JSON serialization
fn bench_json_serialization_pretty(c: &mut Criterion) {
    let instrumentor = Instrumentor::new();

    // Create a medium-sized snapshot
    for i in 0..5 {
        let module_name = format!("module-{}", i);
        let handle = instrumentor.register(&module_name);

        for j in 0..20 {
            let topic = format!("topic-{}", j);
            handle.record_read(&topic, i * 1000 + j * 10);
            handle.record_write(&topic, i * 500 + j * 5);
        }
    }

    let snapshot = instrumentor.collect();

    let mut group = c.benchmark_group("json_serialization_pretty");
    group.bench_function("pretty", |b| {
        b.iter(|| {
            black_box(serde_json::to_string_pretty(&snapshot).unwrap());
        });
    });
    group.finish();
}

/// Benchmark JSON deserialization
fn bench_json_deserialization(c: &mut Criterion) {
    let instrumentor = Instrumentor::new();

    // Create a snapshot and serialize it
    for i in 0..5 {
        let module_name = format!("module-{}", i);
        let handle = instrumentor.register(&module_name);

        for j in 0..20 {
            let topic = format!("topic-{}", j);
            handle.record_read(&topic, i * 1000 + j * 10);
            handle.record_write(&topic, i * 500 + j * 5);
        }
    }

    let snapshot = instrumentor.collect();
    let json = serde_json::to_string(&snapshot).unwrap();

    let mut group = c.benchmark_group("json_deserialization");
    group.bench_function("deserialize", |b| {
        b.iter(|| {
            let _: buswatch_sdk::Snapshot = black_box(serde_json::from_str(&json).unwrap());
        });
    });
    group.finish();
}

/// Benchmark serialization to Vec<u8> (bytes)
fn bench_json_to_vec(c: &mut Criterion) {
    let instrumentor = Instrumentor::new();

    for i in 0..10 {
        let module_name = format!("module-{}", i);
        let handle = instrumentor.register(&module_name);

        for j in 0..50 {
            let topic = format!("topic-{}", j);
            handle.record_read(&topic, i * 1000 + j * 10);
            handle.record_write(&topic, i * 500 + j * 5);
        }
    }

    let snapshot = instrumentor.collect();

    let mut group = c.benchmark_group("json_to_vec");
    group.bench_function("to_vec", |b| {
        b.iter(|| {
            black_box(serde_json::to_vec(&snapshot).unwrap());
        });
    });
    group.finish();
}

/// Benchmark round-trip serialization (serialize + deserialize)
fn bench_json_roundtrip(c: &mut Criterion) {
    let instrumentor = Instrumentor::new();

    for i in 0..5 {
        let module_name = format!("module-{}", i);
        let handle = instrumentor.register(&module_name);

        for j in 0..20 {
            let topic = format!("topic-{}", j);
            handle.record_read(&topic, i * 1000 + j * 10);
            handle.record_write(&topic, i * 500 + j * 5);
        }
    }

    let snapshot = instrumentor.collect();

    let mut group = c.benchmark_group("json_roundtrip");
    group.bench_function("roundtrip", |b| {
        b.iter(|| {
            let json = serde_json::to_string(&snapshot).unwrap();
            let _: buswatch_sdk::Snapshot = serde_json::from_str(&json).unwrap();
        });
    });
    group.finish();
}

/// Benchmark serialization with minimal snapshot (empty)
fn bench_json_serialization_empty(c: &mut Criterion) {
    let instrumentor = Instrumentor::new();
    let snapshot = instrumentor.collect();

    let mut group = c.benchmark_group("json_serialization_empty");
    group.bench_function("empty", |b| {
        b.iter(|| {
            black_box(serde_json::to_string(&snapshot).unwrap());
        });
    });
    group.finish();
}

/// Benchmark serialization with realistic snapshot
fn bench_json_serialization_realistic(c: &mut Criterion) {
    let instrumentor = Instrumentor::new();

    // Simulate a realistic application with multiple services
    let services = vec![
        (
            "order-service",
            vec!["orders.new", "orders.confirmed", "orders.cancelled"],
        ),
        (
            "payment-service",
            vec!["payments.pending", "payments.completed", "payments.failed"],
        ),
        (
            "inventory-service",
            vec!["inventory.reserved", "inventory.released"],
        ),
        (
            "notification-service",
            vec!["notifications.email", "notifications.sms"],
        ),
    ];

    for (service_name, topics) in services {
        let handle = instrumentor.register(service_name);

        for (idx, topic) in topics.iter().enumerate() {
            handle.record_read(topic, (idx as u64 + 1) * 1000);
            handle.record_write(topic, (idx as u64 + 1) * 500);
        }
    }

    let snapshot = instrumentor.collect();
    let json = serde_json::to_string(&snapshot).unwrap();

    let mut group = c.benchmark_group("json_serialization_realistic");
    group.throughput(Throughput::Bytes(json.len() as u64));
    group.bench_function("realistic", |b| {
        b.iter(|| {
            black_box(serde_json::to_string(&snapshot).unwrap());
        });
    });
    group.finish();
}

criterion_group!(
    benches,
    bench_json_serialization_varying_sizes,
    bench_json_serialization_pretty,
    bench_json_deserialization,
    bench_json_to_vec,
    bench_json_roundtrip,
    bench_json_serialization_empty,
    bench_json_serialization_realistic
);
criterion_main!(benches);
