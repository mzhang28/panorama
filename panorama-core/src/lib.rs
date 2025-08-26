mod background;
mod db;
mod server;

use std::ffi::{CStr, c_char};

use anyhow::Result;
use clap::Parser;
use cxx::{CxxString, CxxVector};
use sqlx::{
    SqlitePool,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};
use tokio::runtime::Runtime;

use crate::{db::Dal, server::server_main};

#[cxx::bridge]
mod ffi {
    extern "Rust" {
        unsafe fn run_bridge(argc: i32, argv: *const *const c_char);
    }
}

pub async fn catch(fut: impl std::future::Future<Output = Result<()>>) {
    if let Err(e) = fut.await {
        eprintln!("Error: {e:?}");
    }
}

pub async fn run() -> Result<()> {
    println!("Running daemon from Rust...");

    let pool_opt = SqliteConnectOptions::new()
        .filename("test.db")
        .create_if_missing(true);
    let pool = SqlitePoolOptions::new().connect_with(pool_opt).await?;
    let dal = Dal { pool };
    dal.migrate().await?;

    let background = catch(background::background_loop());
    let server_main_fut = catch(server_main());

    let _ = tokio::join!(background, server_main_fut);
    println!("Done!");

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
    let _ = runtime.block_on(catch(run()));
}
