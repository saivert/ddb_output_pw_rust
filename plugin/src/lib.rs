use std::{ffi::{c_char, c_int, c_void}, sync::Mutex};
use once_cell::sync::Lazy;
use deadbeef_sys::*;

#[macro_use]
mod utils;
use lossycstring::LossyCString;
use utils::*;

mod plugin;
use plugin::*;



#[no_mangle]
///
/// # Safety
/// This is requires since this is a plugin export function
pub unsafe extern "C" fn libdeadbeef_rust_plugin_load(
    api: *const DB_functions_t,
) -> *mut DB_plugin_t {
    let k = |x: i32| {};
    k.call_once(("k"));
    k(2);

    DeadBeef::init_from_ptr::<OutputPlugin>(api)
}
