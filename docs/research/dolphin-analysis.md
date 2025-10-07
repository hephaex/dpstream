# Dolphin Emulator Technical Analysis

## Overview

Dolphin is a GameCube and Wii emulator that enables playing games from these consoles on modern hardware. This analysis focuses on integration aspects relevant to the dpstream project.

## Architecture

### Core Components

```
Dolphin Emulator Architecture
┌─────────────────────────────────────────────┐
│                Core System                  │
│  ┌─────────────┐  ┌─────────────────────┐  │
│  │ PowerPC CPU │  │  Memory Management  │  │
│  │ Emulation   │  │                     │  │
│  │ (Gekko/BW)  │  │  - ARAM, MEM1/MEM2  │  │
│  └─────────────┘  │  - MMU Translation   │  │
│                   │  - Cache Simulation  │  │
│                   └─────────────────────┘  │
└─────────────────────────────────────────────┘
                       │
┌─────────────────────────────────────────────┐
│              Graphics System                │
│  ┌─────────────┐  ┌─────────────────────┐  │
│  │ GPU Plugin  │  │   Video Backend     │  │
│  │ (Flipper/   │  │                     │  │
│  │  Hollywood) │  │ - OpenGL/Vulkan/D3D │  │
│  │             │  │ - Texture Cache     │  │
│  │             │  │ - Shader Generation │  │
│  └─────────────┘  └─────────────────────┘  │
└─────────────────────────────────────────────┘
                       │
┌─────────────────────────────────────────────┐
│               Audio System                  │
│  ┌─────────────┐  ┌─────────────────────┐  │
│  │ DSP Plugin  │  │    Audio Backend    │  │
│  │ (AX/AC-97)  │  │                     │  │
│  │             │  │ - ALSA/PulseAudio   │  │
│  │             │  │ - DirectSound       │  │
│  │             │  │ - CoreAudio         │  │
│  └─────────────┘  └─────────────────────┘  │
└─────────────────────────────────────────────┘
                       │
┌─────────────────────────────────────────────┐
│              Input System                   │
│  ┌─────────────┐  ┌─────────────────────┐  │
│  │ Controller  │  │    Input Backend    │  │
│  │ Emulation   │  │                     │  │
│  │             │  │ - DirectInput       │  │
│  │ - GameCube  │  │ - XInput            │  │
│  │ - Wiimote   │  │ - evdev (Linux)     │  │
│  │ - Nunchuk   │  │ - SDL2              │  │
│  └─────────────┘  └─────────────────────┘  │
└─────────────────────────────────────────────┘
```

## Integration Points

### Process Management

Dolphin can be launched in several modes relevant to streaming:

```bash
# GUI mode (normal operation)
dolphin-emu

# Nogui mode (headless operation)
dolphin-emu --nogui --exec=/path/to/game.iso

# Batch mode (scripted operation)
dolphin-emu --batch --exec=/path/to/game.iso --config=Dolphin.ini
```

### Configuration System

Dolphin uses INI-based configuration files:

```
~/.local/share/dolphin-emu/Config/
├── Dolphin.ini          # Main configuration
├── GFX.ini             # Graphics settings
├── DSP.ini             # Audio settings
├── Logger.ini          # Debug logging
├── Profiles/           # Controller profiles
│   ├── GCPad/
│   └── Wiimote/
└── GameSettings/       # Per-game settings
    ├── GALE01.ini      # Super Smash Bros. Melee
    └── RMCE01.ini      # Mario Kart Wii
```

### Key Configuration Sections

#### Graphics (GFX.ini)
```ini
[Settings]
AspectRatio = 0              # Auto
SafeTextureCache = True
WaitForShaders = True
ShaderCompilationMode = 2    # Async (skip drawing)
EnableGPUTextureDecoding = True
BackgroundCompilation = True

[Enhancements]
ForceFiltering = True
MaxAnisotropy = 4
PostProcessingShader =
StereoMode = 0

[Hacks]
BBoxEnable = False
ForceProgressive = True
SkipEFBCopyToRam = True
```

#### Audio (DSP.ini)
```ini
[DSP]
EnableJIT = True
DumpAudio = False
DumpAudioSilent = False
DumpUCode = False
Backend = Pulse              # Linux
Latency = 20                 # Milliseconds
```

#### Controls
```ini
[Core]
SIDevice0 = 12              # Standard Controller
SIDevice1 = 0               # None
WiimoteSource0 = 1          # Emulated

[Controls]
PadType0 = 0                # Standard
PadProfile0 = default
```

### Window Management

For streaming integration, we need to capture Dolphin's window:

```cpp
// X11 window detection (Linux)
Display* display = XOpenDisplay(NULL);
Window root = DefaultRootWindow(display);

// Find Dolphin window by class or title
Window dolphin_window = find_window_by_class(root, "dolphin-emu");

// Get window geometry
XWindowAttributes attrs;
XGetWindowAttributes(display, dolphin_window, &attrs);
```

### Process Control

```rust
// Rust implementation for process management
use std::process::{Command, Child, Stdio};

pub struct DolphinInstance {
    process: Child,
    window_id: Option<u64>,
}

impl DolphinInstance {
    pub fn launch_game(game_path: &str, config_dir: &str) -> Result<Self> {
        let mut cmd = Command::new("dolphin-emu");
        cmd.arg("--nogui")
           .arg("--exec")
           .arg(game_path)
           .arg("--user")
           .arg(config_dir)
           .stdout(Stdio::piped())
           .stderr(Stdio::piped());

        let process = cmd.spawn()?;

        Ok(Self {
            process,
            window_id: None,
        })
    }
}
```

## Performance Considerations

### CPU Requirements
- **Single-core performance critical**: Dolphin's CPU thread is single-threaded
- **Dual-core recommended**: Separate GPU thread for modern systems
- **AVX2 support**: Significant performance improvement on supported CPUs

### Memory Usage
- **Base usage**: ~200MB for emulator core
- **Game-dependent**: GameCube (16-24MB), Wii (64-88MB + game data)
- **Texture cache**: Can consume 1-4GB depending on settings

### Graphics Performance
- **Backend selection**: Vulkan > D3D12 > OpenGL for performance
- **Shader compilation**: Major source of stuttering, can be pre-cached
- **Resolution scaling**: 2x native = 4x pixel count (1280x1056 GameCube)

## Optimal Settings for Streaming

### Low-Latency Configuration

```ini
# Dolphin.ini
[Core]
CPUThread = True
Fastmem = True
DSPHLE = True
SyncOnSkipIdle = True
SyncGPU = False
SyncGpuMaxDistance = 200000

# GFX.ini
[Settings]
BackendMultithreading = False  # Reduces latency
WaitForShaders = False         # Prevent stalls
ShaderCompilationMode = 1      # Synchronous
VSync = False                  # Let streaming handle sync

# DSP.ini
[DSP]
Latency = 8                    # Minimum latency
```

### Quality vs Performance Presets

```rust
pub enum DolphinPreset {
    Performance,    // 1x native, fast settings
    Balanced,      // 2x native, mixed settings
    Quality,       // 4x native, high settings
    Streaming,     // Optimized for network streaming
}

impl DolphinPreset {
    pub fn configure(&self) -> DolphinConfig {
        match self {
            Self::Streaming => DolphinConfig {
                internal_resolution: (1280, 1056),  // 1x native
                anti_aliasing: AntiAliasing::None,
                anisotropic_filtering: 1,
                wait_for_shaders: false,
                vsync: false,
                audio_latency: 8,
                dual_core: true,
                fast_depth: true,
                skip_efb_copy: true,
            },
            // ... other presets
        }
    }
}
```

## Integration Challenges

### 1. Window Capture Timing
```rust
// Wait for Dolphin window to be ready
async fn wait_for_window(&mut self) -> Result<u64> {
    for attempt in 0..30 {  // 30 second timeout
        if let Some(window_id) = self.find_dolphin_window()? {
            return Ok(window_id);
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    Err(anyhow!("Dolphin window not found"))
}
```

### 2. Game State Management
```rust
pub enum GameState {
    Loading,
    Playing,
    Paused,
    GameOver,
    Menu,
}

impl DolphinInstance {
    pub fn detect_game_state(&self) -> GameState {
        // Use memory scanning or save state inspection
        // This is game-specific and complex
        GameState::Playing  // Placeholder
    }
}
```

### 3. Input Injection
```rust
// Virtual controller implementation
pub struct VirtualController {
    pipe_path: String,
}

impl VirtualController {
    pub fn send_input(&self, input: ControllerState) -> Result<()> {
        // Send input to Dolphin via named pipe or shared memory
        // Dolphin supports input pipes for TAS (Tool-Assisted Speedrun)
        Ok(())
    }
}
```

## Game-Specific Optimizations

### GameCube Games
- **Lower memory requirements**
- **Simpler input (no motion controls)**
- **Better compatibility overall**

### Wii Games
- **Motion control requirements**
- **Higher memory usage**
- **More complex input mapping**

```rust
pub struct GameProfile {
    pub game_id: String,        // "GALE01"
    pub title: String,          // "Super Smash Bros. Melee"
    pub platform: Platform,     // GameCube/Wii
    pub recommended_settings: DolphinConfig,
    pub input_requirements: Vec<InputDevice>,
}

// Example profiles
const MELEE_PROFILE: GameProfile = GameProfile {
    game_id: "GALE01",
    title: "Super Smash Bros. Melee",
    platform: Platform::GameCube,
    recommended_settings: DolphinConfig {
        dual_core: true,
        fast_depth: true,
        // ... optimized for competitive play
    },
    input_requirements: vec![InputDevice::GameCubeController],
};
```

## Monitoring and Diagnostics

### Performance Metrics
```rust
pub struct DolphinMetrics {
    pub fps: f32,
    pub frame_time: Duration,
    pub cpu_usage: f32,
    pub gpu_usage: f32,
    pub memory_usage: u64,
    pub shader_cache_size: u64,
}

impl DolphinInstance {
    pub fn get_metrics(&self) -> Result<DolphinMetrics> {
        // Parse log output or use memory inspection
        // Dolphin outputs performance stats to logs
        Ok(DolphinMetrics::default())
    }
}
```

### Error Handling
```rust
pub enum DolphinError {
    GameNotFound(String),
    ConfigurationError(String),
    GraphicsDriverError(String),
    AudioDeviceError(String),
    ControllerError(String),
    ProcessCrashed(i32),
}
```

## Future Considerations

### 1. Dolphin Integration Improvements
- **Direct API**: Dolphin team working on scripting API
- **Save state streaming**: Remote save state management
- **Netplay integration**: Multiplayer over network

### 2. Performance Optimizations
- **Shader pre-compilation**: Reduce stuttering
- **Memory streaming**: Reduce startup time
- **GPU scheduling**: Better frame pacing

### 3. Feature Extensions
- **Cheat code injection**: Real-time game modifications
- **Replay system**: Record and playback gameplay
- **Achievement tracking**: Custom achievement system

## References

- [Dolphin Emulator Wiki](https://wiki.dolphin-emu.org/)
- [Dolphin Source Code](https://github.com/dolphin-emu/dolphin)
- [GameCube/Wii Technical Documentation](https://wiibrew.org/wiki/Hardware)
- [Dolphin Configuration Guide](https://wiki.dolphin-emu.org/index.php?title=Configuration_Guide)