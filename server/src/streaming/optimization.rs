//! Performance optimization utilities for dpstream server
//!
//! Provides shared optimization patterns and utilities across the streaming pipeline

use crossbeam_channel::{bounded, unbounded, Receiver, Sender};
use parking_lot::Mutex;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

/// Adaptive frame rate controller for quality management
pub struct AdaptiveFrameRateController {
    target_fps: u32,
    current_fps: f64,
    frame_times: VecDeque<Duration>,
    last_adjustment: Instant,
    network_quality: NetworkQuality,
    cpu_usage: f64,
}

#[derive(Debug, Clone, Copy)]
pub enum NetworkQuality {
    Excellent, // < 10ms latency, no packet loss
    Good,      // 10-30ms latency, < 1% packet loss
    Fair,      // 30-50ms latency, 1-3% packet loss
    Poor,      // > 50ms latency, > 3% packet loss
}

#[derive(Debug, Clone, Copy)]
pub struct QualityMetrics {
    pub latency_ms: f64,
    pub packet_loss_percent: f64,
    pub jitter_ms: f64,
    pub bandwidth_kbps: u32,
}

impl AdaptiveFrameRateController {
    pub fn new(target_fps: u32) -> Self {
        Self {
            target_fps,
            current_fps: target_fps as f64,
            frame_times: VecDeque::with_capacity(60),
            last_adjustment: Instant::now(),
            network_quality: NetworkQuality::Good,
            cpu_usage: 0.0,
        }
    }

    pub fn update_metrics(&mut self, metrics: QualityMetrics, cpu_usage: f64) {
        self.cpu_usage = cpu_usage;

        // Determine network quality
        self.network_quality = match (metrics.latency_ms, metrics.packet_loss_percent) {
            (l, p) if l < 10.0 && p < 0.1 => NetworkQuality::Excellent,
            (l, p) if l < 30.0 && p < 1.0 => NetworkQuality::Good,
            (l, p) if l < 50.0 && p < 3.0 => NetworkQuality::Fair,
            _ => NetworkQuality::Poor,
        };

        // Adjust frame rate if needed
        if self.last_adjustment.elapsed() > Duration::from_secs(2) {
            self.adjust_frame_rate();
            self.last_adjustment = Instant::now();
        }
    }

    pub fn record_frame_time(&mut self, frame_time: Duration) {
        self.frame_times.push_back(frame_time);
        if self.frame_times.len() > 60 {
            self.frame_times.pop_front();
        }

        // Calculate current FPS
        if self.frame_times.len() >= 10 {
            let avg_frame_time: Duration =
                self.frame_times.iter().sum::<Duration>() / self.frame_times.len() as u32;
            self.current_fps = 1.0 / avg_frame_time.as_secs_f64();
        }
    }

    fn adjust_frame_rate(&mut self) {
        let adjustment_factor = match (self.network_quality, self.cpu_usage) {
            (NetworkQuality::Excellent, cpu) if cpu < 50.0 => 1.1, // Increase
            (NetworkQuality::Good, cpu) if cpu < 70.0 => 1.0,      // Maintain
            (NetworkQuality::Fair, _) => 0.9,                      // Decrease
            (NetworkQuality::Poor, _) => 0.8,                      // Significant decrease
            (_, cpu) if cpu > 80.0 => 0.85,                        // CPU overload
            _ => 1.0,
        };

        let new_target = (self.target_fps as f64 * adjustment_factor) as u32;
        let new_target = new_target.clamp(15, 120); // Reasonable bounds

        if new_target != self.target_fps {
            info!(
                "Adjusting target FPS from {} to {} (network: {:?}, CPU: {:.1}%)",
                self.target_fps, new_target, self.network_quality, self.cpu_usage
            );
            self.target_fps = new_target;
        }
    }

    pub fn get_target_fps(&self) -> u32 {
        self.target_fps
    }

    pub fn get_current_fps(&self) -> f64 {
        self.current_fps
    }
}

/// Memory-efficient priority queue for frames
pub struct PriorityFrameQueue<T> {
    high_priority: VecDeque<T>,
    normal_priority: VecDeque<T>,
    low_priority: VecDeque<T>,
    max_size: usize,
    dropped_frames: u64,
}

impl<T> PriorityFrameQueue<T> {
    pub fn new(max_size: usize) -> Self {
        Self {
            high_priority: VecDeque::new(),
            normal_priority: VecDeque::new(),
            low_priority: VecDeque::new(),
            max_size,
            dropped_frames: 0,
        }
    }

    pub fn push(&mut self, item: T, priority: u8) {
        // Check if we need to drop frames
        let total_size =
            self.high_priority.len() + self.normal_priority.len() + self.low_priority.len();

        if total_size >= self.max_size {
            // Drop from lowest priority first
            if !self.low_priority.is_empty() {
                self.low_priority.pop_front();
                self.dropped_frames += 1;
            } else if !self.normal_priority.is_empty() {
                self.normal_priority.pop_front();
                self.dropped_frames += 1;
            } else if !self.high_priority.is_empty() {
                self.high_priority.pop_front();
                self.dropped_frames += 1;
            }
        }

        // Add to appropriate queue
        match priority {
            0..=2 => self.low_priority.push_back(item),
            3..=5 => self.normal_priority.push_back(item),
            _ => self.high_priority.push_back(item),
        }
    }

    pub fn pop(&mut self) -> Option<T> {
        // Serve high priority first
        if let Some(item) = self.high_priority.pop_front() {
            return Some(item);
        }

        if let Some(item) = self.normal_priority.pop_front() {
            return Some(item);
        }

        self.low_priority.pop_front()
    }

    pub fn len(&self) -> usize {
        self.high_priority.len() + self.normal_priority.len() + self.low_priority.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn dropped_count(&self) -> u64 {
        self.dropped_frames
    }
}

/// Lock-free statistics collector
pub struct StatsCollector {
    samples: Arc<Mutex<VecDeque<f64>>>,
    max_samples: usize,
}

impl StatsCollector {
    pub fn new(max_samples: usize) -> Self {
        Self {
            samples: Arc::new(Mutex::new(VecDeque::with_capacity(max_samples))),
            max_samples,
        }
    }

    pub fn record(&self, value: f64) {
        let mut samples = self.samples.lock();
        if samples.len() >= self.max_samples {
            samples.pop_front();
        }
        samples.push_back(value);
    }

    pub fn get_average(&self) -> f64 {
        let samples = self.samples.lock();
        if samples.is_empty() {
            return 0.0;
        }
        samples.iter().sum::<f64>() / samples.len() as f64
    }

    pub fn get_percentile(&self, percentile: f64) -> f64 {
        let samples = self.samples.lock();
        if samples.is_empty() {
            return 0.0;
        }

        let mut sorted: Vec<f64> = samples.iter().copied().collect();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let index = ((sorted.len() - 1) as f64 * percentile / 100.0) as usize;
        sorted[index]
    }
}

/// Thread pool for CPU-intensive tasks
pub struct StreamingThreadPool {
    encoding_pool: rayon::ThreadPool,
    processing_pool: rayon::ThreadPool,
}

impl StreamingThreadPool {
    pub fn new() -> Result<Self, rayon::ThreadPoolBuildError> {
        let cpu_count = num_cpus::get();

        // Encoding pool: 50% of cores for heavy encoding tasks
        let encoding_threads = (cpu_count / 2).max(1);
        let encoding_pool = rayon::ThreadPoolBuilder::new()
            .num_threads(encoding_threads)
            .thread_name(|i| format!("encoder-{}", i))
            .build()?;

        // Processing pool: remaining cores for frame processing
        let processing_threads = cpu_count - encoding_threads;
        let processing_pool = rayon::ThreadPoolBuilder::new()
            .num_threads(processing_threads)
            .thread_name(|i| format!("processor-{}", i))
            .build()?;

        info!(
            "Initialized thread pools: {} encoding threads, {} processing threads",
            encoding_threads, processing_threads
        );

        Ok(Self {
            encoding_pool,
            processing_pool,
        })
    }

    pub fn execute_encoding<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        self.encoding_pool.spawn(f);
    }

    pub fn execute_processing<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        self.processing_pool.spawn(f);
    }
}

use std::collections::VecDeque;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_priority_queue() {
        let mut queue = PriorityFrameQueue::new(3);

        queue.push("low1", 1);
        queue.push("normal1", 4);
        queue.push("high1", 7);

        // Should serve high priority first
        assert_eq!(queue.pop(), Some("high1"));
        assert_eq!(queue.pop(), Some("normal1"));
        assert_eq!(queue.pop(), Some("low1"));
    }

    #[test]
    fn test_stats_collector() {
        let collector = StatsCollector::new(100);

        for i in 1..=10 {
            collector.record(i as f64);
        }

        assert_eq!(collector.get_average(), 5.5);
        assert_eq!(collector.get_percentile(50.0), 5.0);
    }

    #[test]
    fn test_adaptive_frame_rate() {
        let mut controller = AdaptiveFrameRateController::new(60);

        let metrics = QualityMetrics {
            latency_ms: 100.0, // Poor network
            packet_loss_percent: 5.0,
            jitter_ms: 20.0,
            bandwidth_kbps: 1000,
        };

        controller.update_metrics(metrics, 50.0);
        // Should reduce target FPS due to poor network
        assert!(controller.get_target_fps() < 60);
    }
}
