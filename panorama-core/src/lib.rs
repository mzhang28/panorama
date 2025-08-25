mod background;
mod db;

use std::ffi::{CStr, c_char};

use anyhow::Result;
use clap::Parser;
use cxx::{CxxString, CxxVector};
use sqlx::SqlitePool;
use tokio::runtime::Runtime;

use crate::db::Dal;

#[cxx::bridge]
mod ffi {
    extern "Rust" {
        unsafe fn run_bridge(argc: i32, argv: *const *const c_char);
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

#[derive(Debug, Parser)]
struct Opt {
    // This will always be on by the time we get to this point.
    #[clap(long = "daemon")]
    daemon: bool,
}

fn run_bridge(argc: i32, argv: *const *const c_char) {
    let mut args = Vec::with_capacity(argc as usize);
    for i in 0..argc {
        let cstr = unsafe { CStr::from_ptr(*argv.add(i as usize)) };
        args.push(cstr.to_string_lossy().into_owned());
    }
    let opt = Opt::parse_from(args);
    println!("Opt: {:?}", opt);

    let runtime = Runtime::new().unwrap();
    let _ = runtime.block_on(run());
}
