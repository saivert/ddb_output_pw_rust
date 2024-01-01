use std::{ffi::{c_char, c_int, c_void}, sync::Mutex};
use once_cell::sync::{Lazy, OnceCell};
use crate::*;
use std::ffi::CString;

pub(crate) struct PluginStructWrapper<'a> {
    pub(crate) id: CString,
    pub(crate) plugin_struct: OnceCell<DB_output_t>,
    pub(crate) plugin: OnceCell<&'a dyn DBOutput>,

}

//pub static mut PLUGIN_WRAPPER: OnceCell<PluginStructWrapper> = OnceCell::default();

pub(crate) static mut PLUGIN: Lazy<Mutex<PluginStructWrapper>> = Lazy::new(|| {
    let wrapper = PluginStructWrapper {
        id: CString::new("Hello").unwrap(),
        plugin_struct: OnceCell::default(),
        plugin: OnceCell::default(),
    };

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
            id: wrapper.id.as_ptr(),
            name: "Pipewire output plugin (rust)\0".as_ptr() as *const i8,
            descr: "This is a new Pipewire based plugin written in rust\0".as_ptr() as *const i8,
            copyright: include_str!("../../LICENSE").as_ptr() as *const i8,
            website: "https://saivert.com".as_ptr() as *const i8,
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
    wrapper.plugin_struct.set(x).unwrap();
    Mutex::new(wrapper)
});


extern "C" fn init() -> c_int {

    println!("rustplug::init");
    unsafe
    {
        if let Ok(p) = &mut PLUGIN.lock() {
            let plugin = p.plugin.get_mut().expect("plugin");
            plugin.as_output().expect("output plugin").init();
        }
    }
    0
}

extern "C" fn free() -> c_int {
    println!("rustplug::free");
    unsafe {
        if let Ok(p) = &mut PLUGIN.lock() {
            let plugin = p.plugin.get_mut().expect("plugin");
            plugin.as_output().expect("output plugin").free();
        }
    }
    0
}

extern "C" fn setformat(fmt: *mut ddb_waveformat_t) -> c_int {
    println!("rustplug::setformat");
    unsafe {
        if let Ok(p) = &mut PLUGIN.lock() {
            let plugin = p.plugin.get_mut().expect("plugin");
            plugin.as_output().expect("output plugin").setformat(*fmt);
        }
    }
    0
}

extern "C" fn play() -> c_int {
    println!("rustplug::play");
    unsafe {
        if let Ok(p) = &mut PLUGIN.lock() {
            let plugin = p.plugin.get_mut().expect("plugin");
            plugin.as_output().expect("output plugin").play();
        }
    }
    0
}


extern "C" fn stop() -> c_int {
    println!("rustplug::stop");
    unsafe {
        if let Ok(p) = &mut PLUGIN.lock() {
            let plugin = p.plugin.get_mut().expect("plugin");
            plugin.as_output().expect("output plugin").stop();
        }
    }
    0
}

extern "C" fn pause() -> c_int {
    println!("rustplug::pause");
    unsafe {
        if let Ok(p) = &mut PLUGIN.lock() {
            let plugin = p.plugin.get_mut().expect("plugin");
            plugin.as_output().expect("output plugin").pause();
        }
    }
    0
}

extern "C" fn unpause() -> c_int {
    println!("rustplug::unpause");
    unsafe {
        if let Ok(p) = &mut PLUGIN.lock() {
            let plugin = p.plugin.get_mut().expect("plugin");
            plugin.as_output().expect("output plugin").unpause();
        }
    }
    0
}

extern "C" fn getstate() -> ddb_playback_state_t {
    unsafe {
        if let Ok(p) = &mut PLUGIN.lock() {
            let plugin = p.plugin.get_mut().expect("plugin");
            plugin.as_output().expect("output plugin").getstate()
        } else {
            DDB_PLAYBACK_STATE_STOPPED
        }
    }
}

extern "C" fn plugin_start() -> c_int {
    unsafe {
        if let Ok(p) = &mut PLUGIN.lock(){
            let plugin = p.plugin.get_mut().expect("plugin");
            plugin.plugin_start();
        }
    }
    0
}

extern "C" fn plugin_stop() -> c_int {
    0
}



extern "C" fn enum_soundcards(
    callback: Option<
        unsafe extern "C" fn(name: *const c_char, desc: *const c_char, _userdata: *mut c_void),
    >,
    userdata: *mut c_void,
) {
    unsafe {
        if let Ok(p) = &mut PLUGIN.lock() {
            let plugin = p.plugin.get_mut().expect("plugin");
            plugin.as_output().expect("output plugin")
            .enum_soundcards_erased(&move|name, desc| {
                let name = LossyCString::new(name);
                let desc = LossyCString::new(desc);
                callback.unwrap()(name.as_ptr(), desc.as_ptr(), userdata);
            });
        }
    }
}


extern "C" fn message(msgid: u32, ctx: usize, p1: u32, p2: u32) -> c_int {
    unsafe {
        if let Ok(p) = PLUGIN.get_mut() {
            let plugin = p.plugin.get_mut().expect("plugin");
            plugin.message(msgid, ctx, p1, p2);
        }
    }
    0
}

