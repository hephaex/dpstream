//! dpstream Switch Client Library
//!
//! A no-std compatible library for Nintendo Switch homebrew that implements
//! a Moonlight-compatible client for streaming games from dpstream servers.

#![no_std]

extern crate alloc;

pub mod error;
pub mod moonlight;
pub mod input;
pub mod display;
pub mod sys;
pub mod network;

pub use error::{Result, ClientError};
pub use moonlight::{MoonlightClient, ClientState, ServerInfo};
pub use input::{InputManager, InputState, Buttons};
pub use display::{DisplayManager, VideoFrame};
pub use sys::LibnxSystem;