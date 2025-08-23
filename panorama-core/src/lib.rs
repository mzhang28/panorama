mod background;
mod db;

use anyhow::Result;
use sqlx::SqlitePool;
use tokio::runtime::Runtime;

use crate::db::Dal;

#[cxx::bridge]
mod ffi {
    extern "Rust" {
        fn run_bridge();
    }
}

pub async fn run() -> Result<()> {
    println!("Running daemon from Rust...");
    tokio::spawn(background::background_loop());

    let pool = SqlitePool::connect("test.db").await?;
    let dal = Dal { pool };
    dal.migrate().await?;

    Ok(())
}

fn run_bridge() {
    let runtime = Runtime::new().unwrap();
    let _ = runtime.block_on(run());
}
