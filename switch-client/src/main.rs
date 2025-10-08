//! Dolphin Remote Gaming Client for Nintendo Switch
//!
//! This homebrew application connects to a dpstream server running on Ubuntu
//! and streams GameCube/Wii games over Tailscale VPN using the Moonlight protocol.
//!
//! Target: Nintendo Switch with Custom Firmware (Atmosphere)
//! Architecture: ARM64 Cortex-A57 (Tegra X1)

#![no_std]
#![no_main]

extern crate alloc;

use alloc::string::String;
use core::panic::PanicInfo;
use linked_list_allocator::LockedHeap;

mod error;
mod moonlight;
mod input;
mod display;
mod sys;
mod network;

use error::{Result, ClientError};
use moonlight::MoonlightClient;
use input::InputManager;
use display::DisplayManager;
use sys::libnx::LibnxSystem;

/// Global allocator for heap memory management
#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

/// Main application state
pub struct DpstreamApp {
    system: LibnxSystem,
    display: DisplayManager,
    input: InputManager,
    moonlight: Option<MoonlightClient>,
    running: bool,
}

impl DpstreamApp {
    /// Initialize the application
    pub fn new() -> Result<Self> {
        // Initialize system services
        let system = LibnxSystem::init()?;

        // Initialize display manager
        let display = DisplayManager::new()?;

        // Initialize input manager
        let input = InputManager::new()?;

        Ok(Self {
            system,
            display,
            input,
            moonlight: None,
            running: true,
        })
    }

    /// Main application loop
    pub fn run(&mut self) -> Result<()> {
        self.display.show_splash_screen()?;

        // Main loop
        while self.running {
            // Update input state
            self.input.update()?;

            // Check for exit condition (Home button)
            if self.input.is_home_pressed() {
                self.running = false;
                break;
            }

            // Handle menu navigation if not connected
            if self.moonlight.is_none() {
                self.handle_menu_input()?;
            } else {
                // Handle game streaming
                self.handle_streaming()?;
            }

            // Update display
            self.display.present_frame()?;
        }

        self.cleanup()?;
        Ok(())
    }

    /// Handle menu input when not streaming
    fn handle_menu_input(&mut self) -> Result<()> {
        if self.input.is_a_pressed() {
            // Connect to server
            self.connect_to_server()?;
        } else if self.input.is_b_pressed() {
            // Show settings menu
            self.display.show_settings_menu()?;
        } else if self.input.is_plus_pressed() {
            // Exit application
            self.running = false;
        }

        Ok(())
    }

    /// Handle streaming input and display
    fn handle_streaming(&mut self) -> Result<()> {
        if let Some(client) = &mut self.moonlight {
            // Send input to server
            let input_state = self.input.get_current_state();
            client.send_input(&input_state)?;

            // Receive and decode video frame
            if let Some(frame) = client.receive_frame()? {
                self.display.render_frame(&frame)?;
            }

            // Check for disconnect
            if self.input.is_minus_pressed() {
                self.disconnect_from_server()?;
            }
        }

        Ok(())
    }

    /// Connect to dpstream server
    fn connect_to_server(&mut self) -> Result<()> {
        self.display.show_connecting_screen()?;

        // Create Moonlight client
        let mut client = MoonlightClient::new()?;

        // For now, mock the connection process
        // In real implementation, this would discover and connect to servers

        self.moonlight = Some(client);
        self.display.show_streaming_ui()?;

        Ok(())
    }

    /// Disconnect from server
    fn disconnect_from_server(&mut self) -> Result<()> {
        if let Some(_client) = self.moonlight.take() {
            // Disconnect logic would go here
        }

        self.display.show_main_menu()?;
        Ok(())
    }

    /// Cleanup resources
    fn cleanup(&mut self) -> Result<()> {
        if let Some(_client) = self.moonlight.take() {
            // Cleanup logic would go here
        }

        self.display.cleanup()?;
        self.input.cleanup()?;
        self.system.cleanup()?;

        Ok(())
    }
}

/// Application entry point
#[no_mangle]
pub extern "C" fn main(_argc: i32, _argv: *const *const u8) -> i32 {
    // Initialize heap allocator
    init_heap();

    // Create and run application
    match run_app() {
        Ok(_) => 0,
        Err(e) => {
            // Log error (in a real implementation, this would use Switch logging)
            // For now, just return error code
            match e {
                ClientError::System(_) => -1,
                ClientError::Network(_) => -2,
                ClientError::Moonlight(_) => -3,
                ClientError::Display(_) => -4,
                ClientError::Input(_) => -5,
                ClientError::Memory(_) => -6,
            }
        }
    }
}

/// Initialize heap allocator
fn init_heap() {
    use linked_list_allocator::LockedHeap;
    use core::mem::MaybeUninit;

    // Allocate 16MB heap (adjust based on available memory)
    const HEAP_SIZE: usize = 16 * 1024 * 1024;
    static mut HEAP: [MaybeUninit<u8>; HEAP_SIZE] = [MaybeUninit::uninit(); HEAP_SIZE];

    unsafe {
        ALLOCATOR.lock().init(HEAP.as_ptr() as *mut u8, HEAP_SIZE);
    }
}

/// Run the main application
fn run_app() -> Result<()> {
    let mut app = DpstreamApp::new()?;
    app.run()
}

/// Panic handler for no_std environment
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    // In a real implementation, this would log to Switch console or file
    // For now, just halt
    loop {}
}

/// Required for no_std + alloc
extern "C" {
    fn abort() -> !;
}

#[no_mangle]
pub extern "C" fn rust_eh_personality() {}

#[no_mangle]
pub extern "C" fn __gxx_personality_v0() {}