//! dpstream Switch Client Library
//!
//! A no-std compatible library for Nintendo Switch homebrew that implements
//! a Moonlight-compatible client for streaming games from dpstream servers.

#![no_std]

extern crate alloc;

pub mod display;
pub mod error;
pub mod input;
pub mod moonlight;
pub mod network;
pub mod sys;

pub use display::{DisplayManager, VideoFrame};
pub use error::{ClientError, Result};
pub use input::{Buttons, InputManager, InputState};
pub use moonlight::{ClientState, MoonlightClient, ServerInfo};
pub use sys::LibnxSystem;
