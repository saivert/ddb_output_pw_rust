use std::ffi::{c_char, c_int, c_void};

use deadbeef_sys::*;

#[macro_use]
mod utils;
use lossycstring::LossyCString;
use utils::*;

mod plugin;
use plugin::*;

static mut PLUGIN: Option<OutputPlugin> = None;

extern "C" fn init() -> c_int {
    println!("rustplug::init");
    unsafe {
        if let Some(p) = &mut PLUGIN {
            p.init();
        }
    }
    0
}

extern "C" fn free() -> c_int {
    println!("rustplug::free");
    unsafe {
        if let Some(p) = &mut PLUGIN {
            p.free();
        }
    }
    0
}

extern "C" fn setformat(fmt: *mut ddb_waveformat_t) -> c_int {
    println!("rustplug::setformat");
    unsafe {
        if let Some(p) = &mut PLUGIN {
            p.setformat(*fmt);
        }
    }
    0
}

extern "C" fn play() -> c_int {
    println!("rustplug::play");
    unsafe {
        if let Some(p) = &mut PLUGIN {
            p.play();
        }
    }
    0
}


extern "C" fn stop() -> c_int {
    println!("rustplug::stop");
    unsafe {
        if let Some(p) = &mut PLUGIN {
            p.stop();
        }
    }
    0
}

extern "C" fn pause() -> c_int {
    println!("rustplug::pause");
    unsafe {
        if let Some(p) = &mut PLUGIN {
            p.pause();
        }
    }
    0
}

extern "C" fn unpause() -> c_int {
    println!("rustplug::unpause");
    unsafe {
        if let Some(p) = &mut PLUGIN {
            p.unpause();
        }
    }
    0
}

extern "C" fn getstate() -> ddb_playback_state_t {
    unsafe {
        if let Some(p) = &PLUGIN {
            p.getstate()
        } else {
            DDB_PLAYBACK_STATE_STOPPED
        }
    }
}

extern "C" fn plugin_start() -> c_int {
    unsafe {
        if let Some(p) = &mut PLUGIN{
            p.plugin_start();
        }
    }
    0
}

extern "C" fn plugin_stop() -> c_int {
    unsafe {
        PLUGIN = None;
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
        if let Some(p) = &PLUGIN {
            
            p.enum_soundcards(move|name, desc| {
                let name = LossyCString::new(name);
                let desc = LossyCString::new(desc);
                callback.unwrap()(name.as_ptr(), desc.as_ptr(), userdata);
            });
        }
    }
}


extern "C" fn message(msgid: u32, ctx: usize, p1: u32, p2: u32) -> c_int {
    unsafe {
        if let Some(p) = PLUGIN.as_mut() {
            p.message(msgid, ctx, p1, p2);
        }
    }
    0
}

#[no_mangle]
///
/// # Safety
/// This is requires since this is a plugin export function
pub unsafe extern "C" fn libdeadbeef_rust_plugin_load(
    api: *const DB_functions_t,
) -> *mut DB_plugin_t {
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
            id: lit_cstr!("pipewirerust"),
            name: lit_cstr!("Pipewire output plugin (rust)"),
            descr: lit_cstr!("This is a new Pipewire based plugin written in rust"),
            copyright: lit_cstr!(include_str!("../LICENSE")),
            website: lit_cstr!("https://saivert.com"),
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

    DeadBeef::init_from_ptr(api);

    PLUGIN = Some(OutputPlugin::new(x));

    let y = PLUGIN.as_mut().unwrap().get_plugin_ptr();

    DeadBeef::set_plugin_ptr(y as *mut DB_plugin_t);

    y as *mut DB_plugin_t
}
