pub(crate) mod builder;
pub(crate) mod config;
pub mod tarn_core;

// Re-export builder types
pub use builder::{BuildError, IndexConfig};

// Re-export for backward compatibility within crate
pub use crate::common;
pub use crate::note_handler;
pub use crate::storage;
