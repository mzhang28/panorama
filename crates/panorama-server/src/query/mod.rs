//! Query engine — compiles Panorama Query Language to parameterized SQL,
//! caches prepared statements, and executes queries against SQLite storage.

pub mod compiler;
pub mod cache;

pub use compiler::compile;
pub use cache::StatementCache;
