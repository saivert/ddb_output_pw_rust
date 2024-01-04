use std::{ffi::{c_char, c_int, c_void, CString}, sync::Mutex};
use once_cell::sync::{Lazy, OnceCell};
use crate::*;

#[derive(Debug)]
pub(crate) struct PluginStructWrapper {
    pub(crate) id: CString,
    pub(crate) name: CString,
    pub(crate) description: CString,
    pub(crate) copyright: CString,
    pub(crate) website: CString,
    pub(crate) plugin: OnceCell<Box<dyn DBPlugin>>,
}

impl std::fmt::Debug for dyn DBPlugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::write(f, format_args!("DBPlugin"))
    }
}

pub(crate) static mut PLUGIN: Lazy<Mutex<OnceCell<PluginStructWrapper>>> = Lazy::new(||Mutex::new(OnceCell::default()));


pub(crate) extern "C" fn init() -> c_int {
    println!("rustplug::init");
    unsafe
    {
        if let Ok(p) = &mut PLUGIN.lock() {
            p.get_mut().expect("PluginStructWrapper")
            .plugin.get_mut().expect("Box<dyn DBPlugin>")
            .as_output().expect("dyn DBOutput")
            .init();
            
        }
    }
    0
}

pub(crate) extern "C" fn free() -> c_int {
    println!("rustplug::free");
    unsafe {
        if let Ok(p) = &mut PLUGIN.lock() {
            p.get_mut().expect("PluginStructWrapper")
            .plugin.get_mut().expect("Box<dyn DBPlugin>")
            .as_output().expect("dyn DBOutput")
            .free();
        }
    }
    0
}

pub(crate) extern "C" fn setformat(fmt: *mut ddb_waveformat_t) -> c_int {
    println!("rustplug::setformat");
    unsafe {
        if let Ok(p) = &mut PLUGIN.lock() {
            p.get_mut().expect("PluginStructWrapper")
            .plugin.get_mut().expect("Box<dyn DBPlugin>")
            .as_output().expect("dyn DBOutput")
            .setformat(*fmt);
        }
    }
    0
}

pub(crate) extern "C" fn play() -> c_int {
    println!("rustplug::play");
    unsafe {
        if let Ok(p) = &mut PLUGIN.lock() {
            p.get_mut().expect("PluginStructWrapper")
            .plugin.get_mut().expect("Box<dyn DBPlugin>")
            .as_output().expect("dyn DBOutput")
            .play();
        }
    }
    0
}


pub(crate) extern "C" fn stop() -> c_int {
    println!("rustplug::stop");
    unsafe {
        if let Ok(p) = &mut PLUGIN.lock() {
            p.get_mut().expect("PluginStructWrapper")
            .plugin.get_mut().expect("Box<dyn DBPlugin>")
            .as_output().expect("dyn DBOutput")
            .stop();
        }
    }
    0
}

pub(crate) extern "C" fn pause() -> c_int {
    println!("rustplug::pause");
    unsafe {
        if let Ok(p) = &mut PLUGIN.lock() {
            p.get_mut().expect("PluginStructWrapper")
            .plugin.get_mut().expect("Box<dyn DBPlugin>")
            .as_output().expect("dyn DBOutput")
            .pause();
        }
    }
    0
}

pub(crate) extern "C" fn unpause() -> c_int {
    println!("rustplug::unpause");
    unsafe {
        if let Ok(p) = &mut PLUGIN.lock() {
            p.get_mut().expect("PluginStructWrapper")
            .plugin.get_mut().expect("Box<dyn DBPlugin>")
            .as_output().expect("dyn DBOutput")
            .unpause();
        }
    }
    0
}

pub(crate) extern "C" fn getstate() -> ddb_playback_state_t {
    unsafe {
        if let Ok(p) = &mut PLUGIN.lock() {
            p.get_mut().expect("PluginStructWrapper")
            .plugin.get_mut().expect("Box<dyn DBPlugin>")
            .as_output().expect("dyn DBOutput")
            .getstate()
        } else {
            DDB_PLAYBACK_STATE_STOPPED
        }
    }
}

pub(crate) extern "C" fn plugin_start() -> c_int {
    unsafe {
        if let Ok(p) = &mut PLUGIN.lock(){
            p.get_mut().expect("PluginStructWrapper")
            .plugin.get_mut().expect("Box<dyn DBPlugin>")
            .plugin_start();
        }
    }
    0
}

pub(crate) extern "C" fn plugin_stop() -> c_int {
    0
}



pub(crate) extern "C" fn enum_soundcards(
    callback: Option<
        unsafe extern "C" fn(name: *const c_char, desc: *const c_char, _userdata: *mut c_void),
    >,
    userdata: *mut c_void,
) {
    unsafe {
        if let Ok(p) = &mut PLUGIN.lock() {
            let cb = EnumSoundcard {
                callback: callback.unwrap(),
                userdata
            };
            p.get_mut().expect("PluginStructWrapper")
            .plugin.get_mut().expect("Box<dyn DBPlugin>")
            .as_output().expect("dyn DBOutput")
            .enum_soundcards(cb);
        }
    }
}


pub(crate) extern "C" fn message(msgid: u32, ctx: usize, p1: u32, p2: u32) -> c_int {
    unsafe {
        if let Ok(p) = &mut PLUGIN.lock() {
            p.get_mut().expect("PluginStructWrapper")
            .plugin.get_mut().expect("Box<dyn DBPlugin>")
            .message(msgid, ctx, p1, p2)
        }
    }
    0
}

