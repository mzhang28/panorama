//! Query engine — compiles Panorama Query Language to parameterized SQL,
//! caches prepared statements, and executes queries against SQLite storage.

pub mod cache;
pub mod compiler;
pub mod physical;

pub use cache::StatementCache;
pub use compiler::compile;
