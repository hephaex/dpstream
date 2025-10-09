//! Performance benchmarks for dpstream server
//!
//! Benchmarks key performance-critical operations

use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId, black_box};
use std::time::Duration;
use std::sync::Arc;
use dashmap::DashMap;
use flume::{bounded, unbounded};
use cache_padded::CachePadded;
use std::sync::atomic::{AtomicU64, Ordering};

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

fn bench_streaming_performance(c: &mut Criterion) {
    let mut group = c.benchmark_group("streaming_performance");

    // Mock video frame data
    let frame_data_720p = vec![0u8; 1280 * 720 * 3]; // RGB data
    let frame_data_1080p = vec![0u8; 1920 * 1080 * 3]; // RGB data

    group.bench_function("video_frame_copy_720p", |b| {
        b.iter(|| {
            let copied = frame_data_720p.clone();
            criterion::black_box(copied);
        });
    });

    group.bench_function("video_frame_copy_1080p", |b| {
        b.iter(|| {
            let copied = frame_data_1080p.clone();
            criterion::black_box(copied);
        });
    });

    group.bench_function("rtp_packet_parsing", |b| {
        // Mock RTP packet
        let rtp_packet = vec![
            0x80, 0x60, 0x00, 0x01, // Version, PT, Sequence
            0x00, 0x00, 0x00, 0x10, // Timestamp
            0x12, 0x34, 0x56, 0x78, // SSRC
            0x48, 0x65, 0x6c, 0x6c, 0x6f // Payload
        ];

        b.iter(|| {
            // Mock parsing logic
            let version = (rtp_packet[0] >> 6) & 0x03;
            let payload_type = rtp_packet[1] & 0x7F;
            let sequence = u16::from_be_bytes([rtp_packet[2], rtp_packet[3]]);
            criterion::black_box((version, payload_type, sequence));
        });
    });

    group.finish();
}

fn bench_optimized_data_structures(c: &mut Criterion) {
    use dashmap::DashMap;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    let mut group = c.benchmark_group("data_structures");

    // Standard HashMap with Mutex
    let std_map = Arc::new(Mutex::new(HashMap::<u64, String>::new()));
    // DashMap (concurrent)
    let dash_map = Arc::new(DashMap::<u64, String>::new());

    group.bench_function("hashmap_insert_mutex", |b| {
        let map = std_map.clone();
        b.iter(|| {
            let mut guard = map.lock().unwrap();
            for i in 0..100 {
                guard.insert(i, format!("value_{}", i));
            }
            guard.clear();
        });
    });

    group.bench_function("dashmap_insert", |b| {
        let map = dash_map.clone();
        b.iter(|| {
            for i in 0..100 {
                map.insert(i, format!("value_{}", i));
            }
            map.clear();
        });
    });

    group.finish();
}

/// Benchmark cache-aligned performance optimizations
fn bench_cache_optimization(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_optimization");

    // Benchmark cache-padded vs non-padded atomic operations
    group.bench_function("atomic_unpadded", |b| {
        let counter = AtomicU64::new(0);
        b.iter(|| {
            for _ in 0..1000 {
                counter.fetch_add(1, Ordering::Relaxed);
                black_box(counter.load(Ordering::Relaxed));
            }
        });
    });

    group.bench_function("atomic_cache_padded", |b| {
        let counter = CachePadded::new(AtomicU64::new(0));
        b.iter(|| {
            for _ in 0..1000 {
                counter.fetch_add(1, Ordering::Relaxed);
                black_box(counter.load(Ordering::Relaxed));
            }
        });
    });

    group.finish();
}

/// Benchmark channel performance for different types
fn bench_channel_performance(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut group = c.benchmark_group("channel_performance");

    group.bench_function("flume_bounded", |b| {
        b.to_async(&rt).iter(|| async {
            let (tx, rx) = bounded(1000);

            // Producer
            for i in 0..1000 {
                tx.send_async(i).await.unwrap();
            }

            // Consumer
            for _ in 0..1000 {
                black_box(rx.recv_async().await.unwrap());
            }
        });
    });

    group.bench_function("flume_unbounded", |b| {
        b.to_async(&rt).iter(|| async {
            let (tx, rx) = unbounded();

            // Producer
            for i in 0..1000 {
                tx.send_async(i).await.unwrap();
            }

            // Consumer
            for _ in 0..1000 {
                black_box(rx.recv_async().await.unwrap());
            }
        });
    });

    group.finish();
}

/// Benchmark enterprise-scale concurrent operations
fn bench_enterprise_concurrency(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut group = c.benchmark_group("enterprise_concurrency");

    // Simulate high-throughput session management
    group.bench_function("concurrent_session_management", |b| {
        b.to_async(&rt).iter(|| async {
            let sessions = Arc::new(DashMap::new());

            // Simulate concurrent session operations
            let tasks: Vec<_> = (0..100)
                .map(|i| {
                    let sessions = sessions.clone();
                    tokio::spawn(async move {
                        // Insert session
                        sessions.insert(i, format!("session_{}", i));

                        // Read session
                        black_box(sessions.get(&i));

                        // Update session
                        sessions.insert(i, format!("updated_session_{}", i));

                        // Remove session
                        sessions.remove(&i);
                    })
                })
                .collect();

            for task in tasks {
                task.await.unwrap();
            }
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_memory_allocation,
    bench_network_operations,
    bench_serialization,
    bench_async_operations,
    bench_streaming_performance,
    bench_optimized_data_structures,
    bench_cache_optimization,
    bench_channel_performance,
    bench_enterprise_concurrency
);
criterion_main!(benches);