use std::io::{stdout, Write};

use anyhow::Result;
use chrono::{DateTime, Utc};
use wasmtime::{Caller, InstancePre, Linker, Memory};

pub struct WasmtimeModule {
  pub(crate) module: InstancePre<WasmtimeInstanceEnv>,
}

impl WasmtimeModule {
  pub fn link_imports(linker: &mut Linker<WasmtimeInstanceEnv>) -> Result<()> {
    macro_rules! link_function {
      ($($module:literal :: $func:ident),* $(,)?) => {
        linker $(
          .func_wrap(
            $module,
            concat!("_", stringify!($func)),
            WasmtimeInstanceEnv::$func,
          )?
        )*;
      };
    }
    abi_funcs!(link_function);
    Ok(())
  }
}

/// This is loosely based on SpacetimeDB's implementation of their host.
/// See: https://github.com/clockworklabs/SpacetimeDB/blob/c19c0d45c454db2a4215deb23c7f9f82cb5d7561/crates/core/src/host/wasmtime/wasm_instance_env.rs
pub struct WasmtimeInstanceEnv {
  /// This is only an Option because memory is initialized after this is created so we need to come back and put it in later
  pub(crate) mem: Option<Memory>,
}

impl WasmtimeInstanceEnv {
  pub fn print(mut caller: Caller<'_, Self>, len: u64, ptr: u32) {
    let mem = caller.data().mem.unwrap();
    let mut buffer = vec![0; len as usize];
    mem.read(caller, ptr as usize, &mut buffer);
    let s = String::from_utf8(buffer).unwrap();
    println!("Called print: {}", s);
  }

  pub fn get_current_time(_: Caller<'_, Self>) -> i64 {
    let now = Utc::now();
    now.timestamp_nanos_opt().unwrap()
  }

  pub fn register_endpoint(mut caller: Caller<'_, Self>) {}
}
