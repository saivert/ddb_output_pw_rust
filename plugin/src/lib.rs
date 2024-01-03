use deadbeef_sys::*;

#[macro_use]
mod utils;
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
    DeadBeef::create_output_plugin::<OutputPlugin>(
        api,
        "rustplug",
        "Pipewire output xx(rust)",
        "Output plugin for PipeWire written in rust",
        include_str!("../../LICENSE"),
        "https://saivert.com",
    )
}
