//! GL.Inet router DNS client.
//!
//! Provides DNS management for GL.Inet routers (e.g., Beryl AX)
//! via their custom hosts feature.

mod client;
mod types;

pub use client::GlInetClient;
pub use types::GlInetDnsHost;
