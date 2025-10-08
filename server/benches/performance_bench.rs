//! Performance benchmarks for dpstream server
//!
//! Benchmarks key performance-critical operations

use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};
use std::time::Duration;

fn bench_memory_allocation(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_allocation");

    for size in [1024, 4096, 16384, 65536].iter() {
        group.bench_with_input(BenchmarkId::new("vec_allocation", size), size, |b, &size| {
            b.iter(|| {
                let vec: Vec<u8> = vec![0; size];
                criterion::black_box(vec);
            });
        });
    }

    group.finish();
}

fn bench_network_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("network_operations");

    group.bench_function("ip_parsing", |b| {
        b.iter(|| {
            let ip = "192.168.1.1".parse::<std::net::IpAddr>();
            criterion::black_box(ip);
        });
    });

    group.bench_function("hostname_resolution", |b| {
        b.iter(|| {
            let hostname = hostname::get();
            criterion::black_box(hostname);
        });
    });

    group.finish();
}

fn bench_serialization(c: &mut Criterion) {
    use serde_json;

    let mut group = c.benchmark_group("serialization");

    #[derive(serde::Serialize, serde::Deserialize)]
    struct TestData {
        id: u64,
        name: String,
        values: Vec<f64>,
    }

    let test_data = TestData {
        id: 12345,
        name: "benchmark_test".to_string(),
        values: vec![1.0, 2.0, 3.0, 4.0, 5.0],
    };

    group.bench_function("json_serialize", |b| {
        b.iter(|| {
            let json = serde_json::to_string(&test_data);
            criterion::black_box(json);
        });
    });

    let json_data = serde_json::to_string(&test_data).unwrap();
    group.bench_function("json_deserialize", |b| {
        b.iter(|| {
            let data: Result<TestData, _> = serde_json::from_str(&json_data);
            criterion::black_box(data);
        });
    });

    group.finish();
}

fn bench_async_operations(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    let mut group = c.benchmark_group("async_operations");

    group.bench_function("tokio_sleep", |b| {
        b.to_async(&rt).iter(|| async {
            tokio::time::sleep(Duration::from_nanos(1)).await;
        });
    });

    group.bench_function("tokio_spawn", |b| {
        b.to_async(&rt).iter(|| async {
            let handle = tokio::spawn(async {
                42
            });
            let result = handle.await;
            criterion::black_box(result);
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_memory_allocation,
    bench_network_operations,
    bench_serialization,
    bench_async_operations
);
criterion_main!(benches);