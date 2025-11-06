//! Agent session manager implementation
//!
//! This module is organized into logical submodules:
//! - `core`: Core struct, constructors, and lifecycle management
//! - `spawn`: Session spawning logic
//! - `info`: Session information queries
//! - `output`: Output retrieval with pagination
//! - `list`: Session listing
//! - `interaction`: Message sending and termination
//! - `pagination`: Pagination utilities

// Module declarations
mod core;
mod spawn;
mod info;
mod output;
mod list;
mod interaction;
mod pagination;

// Re-export public API
pub use core::AgentManager;
pub use spawn::SpawnSessionRequest;
