//! RouterOS REST API client and types.

mod client;
mod types;

pub use client::RouterOsClient;
pub use types::{RouterOsDnsRecord, RouterOsDnsRecordPut};
