mod client;
mod cluster;
mod credentials;
pub mod parser;

pub use client::MySQLTool;
pub use cluster::{Backend, ClusterInfo, Frontend};
pub use credentials::CredentialManager;
pub use parser::{parse_backends, parse_frontends};

/// System databases to hide from selection
pub const SYSTEM_DATABASES: &[&str] = &["__internal_schema", "mysql", "information_schema"];
