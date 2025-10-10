//! Comprehensive integration tests for dpstream server
//!
//! Tests end-to-end functionality including streaming, input handling, and client management

use std::time::Duration;
use tokio::time::timeout;
use uuid::Uuid;

mod common;

use common::*;
use dpstream_server::{
    error::Result,
    input::ServerInputManager,
    streaming::{MoonlightServer, ServerConfig},
};

/// Test server initialization and basic functionality
#[tokio::test]
async fn test_server_initialization() -> Result<()> {
    let config = ServerConfig {
        bind_addr: "127.0.0.1".to_string(),
        port: 0, // Let OS choose port
        max_clients: 4,
        enable_encryption: false, // Disable for testing
        enable_authentication: false,
        stream_timeout_ms: 5000,
    };

    let server = MoonlightServer::new(config).await?;
    assert!(server.port() > 0);

    Ok(())
}

/// Test client connection and session management
#[tokio::test]
async fn test_client_session_management() -> Result<()> {
    let mut test_env = TestEnvironment::new().await?;

    // Test client connection
    let client_id = test_env.connect_client("test_client_1").await?;
    assert_eq!(test_env.get_active_sessions().len(), 1);

    // Test multiple clients
    let client_id_2 = test_env.connect_client("test_client_2").await?;
    assert_eq!(test_env.get_active_sessions().len(), 2);

    // Test client disconnection
    test_env.disconnect_client(&client_id).await?;
    assert_eq!(test_env.get_active_sessions().len(), 1);

    test_env.disconnect_client(&client_id_2).await?;
    assert_eq!(test_env.get_active_sessions().len(), 0);

    Ok(())
}

/// Test video streaming pipeline end-to-end
#[tokio::test]
async fn test_video_streaming_pipeline() -> Result<()> {
    let mut test_env = TestEnvironment::new().await?;
    let client_id = test_env.connect_client("streaming_test").await?;

    // Start streaming session
    test_env.start_streaming(&client_id).await?;

    // Generate test video frames
    let test_frames = generate_test_video_frames(720, 480, 30); // 30 frames at 720x480

    // Send frames through pipeline
    for frame in test_frames {
        test_env.send_video_frame(frame).await?;
    }

    // Verify frames were processed
    let received_frames = test_env.get_received_frames(&client_id).await?;
    assert!(received_frames.len() >= 25); // Allow for some frame drops

    // Verify frame quality metrics
    let quality_metrics = test_env.get_quality_metrics(&client_id).await?;
    assert!(quality_metrics.avg_latency_ms < 50.0);
    assert!(quality_metrics.frame_drop_rate < 0.1); // Less than 10% drops

    Ok(())
}

/// Test audio streaming and synchronization
#[tokio::test]
async fn test_audio_streaming() -> Result<()> {
    let mut test_env = TestEnvironment::new().await?;
    let client_id = test_env.connect_client("audio_test").await?;

    test_env.start_streaming(&client_id).await?;

    // Generate test audio samples
    let audio_samples = generate_test_audio_samples(48000, 2, Duration::from_secs(1));

    // Send audio through pipeline
    for sample in audio_samples {
        test_env.send_audio_sample(sample).await?;
    }

    // Verify audio processing
    let audio_metrics = test_env.get_audio_metrics(&client_id).await?;
    assert!(audio_metrics.latency_ms < 30.0);
    assert!(audio_metrics.buffer_underruns == 0);
    assert!(audio_metrics.sample_rate == 48000);

    Ok(())
}

/// Test input handling and controller mapping
#[tokio::test]
async fn test_input_processing() -> Result<()> {
    let mut test_env = TestEnvironment::new().await?;
    let client_id = test_env.connect_client("input_test").await?;

    test_env.start_streaming(&client_id).await?;

    // Simulate controller inputs
    let test_inputs = vec![
        create_button_input(0x1000, true), // A button press
        create_analog_input(-32768, 0),    // Left stick left
        create_trigger_input(255, 0),      // Left trigger full
    ];

    for input in test_inputs {
        test_env.send_input(&client_id, input).await?;
    }

    // Verify inputs were processed and mapped correctly
    let processed_inputs = test_env.get_processed_inputs().await?;
    assert_eq!(processed_inputs.len(), 3);

    // Verify Dolphin commands were generated
    let dolphin_commands = test_env.get_dolphin_commands().await?;
    assert!(dolphin_commands.len() >= 3);

    Ok(())
}

/// Test network resilience and error recovery
#[tokio::test]
async fn test_network_resilience() -> Result<()> {
    let mut test_env = TestEnvironment::new().await?;
    let client_id = test_env.connect_client("resilience_test").await?;

    test_env.start_streaming(&client_id).await?;

    // Simulate network issues
    test_env.simulate_packet_loss(0.05).await?; // 5% packet loss
    test_env.simulate_latency_spike(100).await?; // 100ms spike

    // Continue streaming during network issues
    let test_frames = generate_test_video_frames(1280, 720, 60);
    for frame in test_frames.into_iter().take(60) {
        // 1 second worth
        test_env.send_video_frame(frame).await?;
        tokio::time::sleep(Duration::from_millis(16)).await; // ~60 FPS
    }

    // Verify system recovered
    let final_metrics = test_env.get_quality_metrics(&client_id).await?;
    assert!(final_metrics.connection_stability > 0.9); // 90% stable

    // Reset network conditions
    test_env.reset_network_simulation().await?;

    Ok(())
}

/// Test concurrent multiple client sessions
#[tokio::test]
async fn test_concurrent_clients() -> Result<()> {
    let mut test_env = TestEnvironment::new().await?;

    // Connect multiple clients
    let mut client_ids = Vec::new();
    for i in 0..4 {
        let client_id = test_env.connect_client(&format!("client_{}", i)).await?;
        client_ids.push(client_id);
    }

    // Start streaming for all clients
    for client_id in &client_ids {
        test_env.start_streaming(client_id).await?;
    }

    // Generate concurrent load
    let frame_tasks = client_ids
        .iter()
        .map(|client_id| {
            let env = test_env.clone();
            let id = *client_id;
            tokio::spawn(async move {
                let frames = generate_test_video_frames(1280, 720, 30);
                for frame in frames {
                    env.send_video_frame_to_client(&id, frame).await.unwrap();
                    tokio::time::sleep(Duration::from_millis(33)).await; // ~30 FPS
                }
            })
        })
        .collect::<Vec<_>>();

    // Wait for all tasks to complete
    for task in frame_tasks {
        task.await.unwrap();
    }

    // Verify all sessions maintained quality
    for client_id in &client_ids {
        let metrics = test_env.get_quality_metrics(client_id).await?;
        assert!(metrics.avg_latency_ms < 100.0);
        assert!(metrics.frame_drop_rate < 0.2); // Allow higher drops under load
    }

    Ok(())
}

/// Test server shutdown and cleanup
#[tokio::test]
async fn test_graceful_shutdown() -> Result<()> {
    let mut test_env = TestEnvironment::new().await?;

    // Connect clients and start streaming
    let client_id_1 = test_env.connect_client("shutdown_test_1").await?;
    let client_id_2 = test_env.connect_client("shutdown_test_2").await?;

    test_env.start_streaming(&client_id_1).await?;
    test_env.start_streaming(&client_id_2).await?;

    // Initiate graceful shutdown
    let shutdown_result = timeout(Duration::from_secs(10), test_env.shutdown()).await;
    assert!(shutdown_result.is_ok());

    // Verify all resources were cleaned up
    assert_eq!(test_env.get_active_sessions().len(), 0);
    assert!(test_env.is_fully_shutdown().await);

    Ok(())
}

/// Test performance under load
#[tokio::test]
async fn test_performance_benchmarks() -> Result<()> {
    let mut test_env = TestEnvironment::new().await?;
    let client_id = test_env.connect_client("perf_test").await?;

    test_env.start_streaming(&client_id).await?;

    // High-throughput test
    let start_time = std::time::Instant::now();
    let test_frames = generate_test_video_frames(1920, 1080, 120); // 2 seconds at 60 FPS

    for frame in test_frames {
        test_env.send_video_frame(frame).await?;
    }

    let elapsed = start_time.elapsed();
    let fps = 120.0 / elapsed.as_secs_f64();

    // Verify performance targets
    assert!(fps >= 55.0); // Should maintain at least 55 FPS under load

    let memory_usage = test_env.get_memory_usage().await?;
    assert!(memory_usage.heap_usage_mb < 512); // Less than 512MB

    Ok(())
}

/// Test error scenarios and recovery
#[tokio::test]
async fn test_error_scenarios() -> Result<()> {
    let mut test_env = TestEnvironment::new().await?;

    // Test invalid client connection
    let invalid_result = test_env.connect_invalid_client().await;
    assert!(invalid_result.is_err());

    // Test resource exhaustion
    test_env.simulate_memory_pressure().await?;
    let client_id = test_env.connect_client("error_test").await?;

    // Should handle gracefully under pressure
    let streaming_result = test_env.start_streaming(&client_id).await;
    assert!(streaming_result.is_ok() || test_env.has_graceful_degradation().await);

    Ok(())
}
