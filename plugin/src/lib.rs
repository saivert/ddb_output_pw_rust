use std::{ffi::{c_char, c_int, c_void}, sync::Mutex};
use once_cell::sync::Lazy;
use deadbeef_sys::*;

#[macro_use]
mod utils;
use lossycstring::LossyCString;
use utils::*;

mod plugin;
use plugin::*;

unsafe impl Send for OutputPlugin {}

static PLUGIN: Lazy<Mutex<OutputPlugin>> = Lazy::new(|| {
    let x = DB_output_t {
        init: Some(init),
        free: Some(free),
        play: Some(play),
        stop: Some(stop),
        pause: Some(pause),
        unpause: Some(unpause),
        enum_soundcards: Some(enum_soundcards),
        setformat: Some(setformat),
        state: Some(getstate),
        has_volume: 1,

        fmt: ddb_waveformat_t::default(),

        plugin: DB_plugin_t {
            api_vmajor: 1,
            api_vminor: 0,
            version_major: 0,
            version_minor: 1,
            flags: DDB_PLUGIN_FLAG_LOGGING,
            type_: DB_PLUGIN_OUTPUT as i32,
            id: c"pipewirerust".as_ptr(),
            name: c"Pipewire output plugin (rust)".as_ptr(),
            descr: c"This is a new Pipewire based plugin written in rust".as_ptr(),
            copyright: concat!(include_str!("../../LICENSE"),"\0").as_ptr() as *const i8,
            website: c"https://saivert.com".as_ptr(),
            start: Some(plugin_start),
            stop: Some(plugin_stop),
            message: Some(message),
            connect: None,
            get_actions: None,
            exec_cmdline: None,
            disconnect: None,
            command: None,
            configdialog: std::ptr::null(),
            reserved1: 0,
            reserved2: 0,
            reserved3: 0,
        },
    };
    Mutex::new(OutputPlugin::new(x))
});

extern "C" fn init() -> c_int {
    debug!("rustplug::init");
    if let Ok(mut p) = PLUGIN.lock() {
        p.init();
    }
    0
}

extern "C" fn free() -> c_int {
    debug!("rustplug::free");
    if let Ok(mut p) = PLUGIN.lock() {
        p.free();
    }
    0
}

extern "C" fn setformat(fmt: *mut ddb_waveformat_t) -> c_int {
    debug!("rustplug::setformat");
    if let Ok(mut p) = PLUGIN.lock() {
        unsafe { p.setformat(*fmt); }
    }
    0
}

extern "C" fn play() -> c_int {
    debug!("rustplug::play");
    if let Ok(mut p) = PLUGIN.lock() {
        p.play();
    }
    0
}


extern "C" fn stop() -> c_int {
    debug!("rustplug::stop");
    if let Ok(mut p) = PLUGIN.lock() {
        p.stop();
    }
    0
}

extern "C" fn pause() -> c_int {
    debug!("rustplug::pause");
    if let Ok(mut p) = PLUGIN.lock() {
        p.pause();
    }
    0
}

extern "C" fn unpause() -> c_int {
    debug!("rustplug::unpause");
    if let Ok(mut p) = PLUGIN.lock() {
        p.unpause();
    }
    0
}

extern "C" fn getstate() -> ddb_playback_state_t {
    if let Ok(p) = PLUGIN.lock() {
        p.getstate()
    } else {
        DDB_PLAYBACK_STATE_STOPPED
    }
}

extern "C" fn plugin_start() -> c_int {
    if let Ok(mut p) = PLUGIN.lock(){
        p.plugin_start();
    }
    0
}

extern "C" fn plugin_stop() -> c_int {
    if let Ok(mut p) = PLUGIN.lock(){
        p.plugin_stop();
    }
    0
}

extern "C" fn enum_soundcards(
    callback: Option<
        unsafe extern "C" fn(name: *const c_char, desc: *const c_char, _userdata: *mut c_void),
    >,
    userdata: *mut c_void,
) {
    unsafe {
        if let Ok(p) = &PLUGIN.lock() {
            
            p.enum_soundcards(move|name, desc| {
                let name = LossyCString::new(name);
                let desc = LossyCString::new(desc);
                callback.unwrap()(name.as_ptr(), desc.as_ptr(), userdata);
            });
        }
    }
}


extern "C" fn message(msgid: u32, ctx: usize, p1: u32, p2: u32) -> c_int {
    if let Ok(mut p) = PLUGIN.lock() {
        p.message(msgid, ctx, p1, p2);
    }
    0
}

#[no_mangle]
///
/// # Safety
/// This is required since this is a plugin export function
pub unsafe extern "C" fn ddb_output_pw_rust_load(
    api: *const DB_functions_t,
) -> *mut DB_plugin_t {
    DeadBeef::init_from_ptr(api, &*PLUGIN.lock().unwrap())
}
