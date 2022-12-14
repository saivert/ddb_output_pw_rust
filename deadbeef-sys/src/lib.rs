#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]
#![allow(clippy::all)]

use lossycstring::LossyCString;

use std::ffi::c_void;

static mut DEADBEEF: Option<DeadBeef> = None;
static mut DEADBEEF_THREAD_ID: Option<std::thread::ThreadId> = None;

#[allow(deref_nullptr)]
mod api {
include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
}
pub use api::*;
/// Main DeadBeef struct that encapsulates common DeadBeef API functions.
pub struct DeadBeef {
    pub(crate) ptr: *const DB_functions_t,
    pub(crate) plugin_ptr: *mut DB_plugin_t,
}

pub trait DBPlugin {
    fn new(plugin: DB_output_t) -> Self;
    fn get_plugin_ptr(&mut self) -> *mut c_void;
    fn plugin_start(&mut self);
    fn plugin_stop(&mut self);
}

pub trait DBOutput: DBPlugin {
    fn init(&mut self) -> i32 {0}
    fn free(&mut self);
    fn play(&mut self);
    fn stop(&mut self);
    fn pause(&mut self);
    fn unpause(&mut self);
    fn getstate(&self) -> ddb_playback_state_e;
    fn setformat(&mut self, fmt: ddb_waveformat_t);
    fn message(&mut self, msgid: u32, ctx: usize, p1: u32, p2: u32);
    fn enum_soundcards<F>(&self, callback: F) where F: Fn(&str, &str) + 'static;
}

impl DeadBeef {
    pub unsafe fn init_from_ptr(api: *const DB_functions_t, plugin: *mut DB_plugin_t) -> DeadBeef {
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
        let len = buf.iter().position(|&c| c == 0).expect("buffer overflow in conf_get_str");
        buf.truncate(len);
        String::from_utf8_lossy(&buf).to_string()
    }

    pub fn volume_set_amp(vol: f32) {
        let deadbeef = unsafe { DeadBeef::deadbeef() };
        let volume_set_amp = deadbeef.get().volume_set_amp.unwrap();

        unsafe { volume_set_amp(vol); }
    }

    pub fn volume_get_amp() -> f32 {
        let deadbeef = unsafe { DeadBeef::deadbeef() };
        let volume_get_amp = deadbeef.get().volume_get_amp.unwrap();

        unsafe { volume_get_amp() }
    }


}
