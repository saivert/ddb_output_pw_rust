#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

#![allow(dead_code)]
#![allow(clippy::all)]
use crate::{DEADBEEF, DEADBEEF_THREAD_ID, utils::LossyCString};
use std::ffi::c_void;

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

/// Main DeadBeef struct that encapsulates common DeadBeef API functions.
pub struct DeadBeef {
    pub(crate) ptr: *const DB_functions_t,
    pub(crate) plugin_ptr: *mut DB_output_t,
}

impl DeadBeef {
    pub unsafe fn init_from_ptr(api: *const DB_functions_t, plugin: *mut DB_output_t) -> DeadBeef {
        assert!(!api.is_null());

        DEADBEEF = Some(DeadBeef { ptr: api, plugin_ptr: plugin });
        DEADBEEF_THREAD_ID = Some(std::thread::current().id());

        DeadBeef { ptr: api, plugin_ptr: plugin }
    }

    pub unsafe fn deadbeef() -> &'static mut DeadBeef {
        match DEADBEEF {
            Some(ref mut w) => w,
            None => panic!("Plugin wasn't initialized correctly"),
        }
    }

    #[inline]
    pub(crate) fn get(&self) -> &DB_functions_t {
        unsafe { &*self.ptr }
    }

    pub fn sendmessage(msg: u32, ctx: usize, p1: u32, p2: u32) -> i32 {
        let deadbeef = unsafe { DeadBeef::deadbeef() };

        let sendmessage = deadbeef.get().sendmessage.unwrap();

        unsafe { sendmessage(msg, ctx, p1, p2) }
    }

    pub fn log_detailed(layers: u32, msg: &str) {
        let deadbeef = unsafe { DeadBeef::deadbeef() };
        let log_detailed = deadbeef.get().log_detailed.unwrap();
        let msg = LossyCString::new(msg);
        unsafe {
            log_detailed(deadbeef.plugin_ptr as *mut DB_plugin_t, layers, msg.as_ptr());
        }
    }

    pub fn streamer_read(buf: *mut c_void, len: usize) -> i32 {
        let deadbeef = unsafe { DeadBeef::deadbeef() };

        let streamer_read = deadbeef.get().streamer_read.unwrap();

        unsafe { streamer_read(buf as *mut i8 , len as i32) }
    }

    pub fn streamer_ok_to_read(len: i32) -> i32 {
        let deadbeef = unsafe { DeadBeef::deadbeef() };

        let streamer_ok_to_read = deadbeef.get().streamer_ok_to_read.unwrap();

        unsafe { streamer_ok_to_read(len as i32) }

    }

    pub fn conf_get_str(item: impl Into<String>, default: impl Into<String>) -> String {
        let deadbeef = unsafe { DeadBeef::deadbeef() };

        let item = LossyCString::new(item.into());
        let default = LossyCString::new(default.into());
        let conf_get_str = deadbeef.get().conf_get_str.unwrap();
        let mut buf: Vec<u8> = vec![0; 4096];

        unsafe { conf_get_str(item.as_ptr(), default.as_ptr(), buf.as_mut_ptr() as *mut i8, 4096); }

        String::from_utf8_lossy(&buf).trim_end_matches(char::from(0)).to_string()
    }

}
