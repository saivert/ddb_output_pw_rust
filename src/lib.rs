#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![deny(elided_lifetimes_in_paths)]

use std::ffi::{c_char, c_int, c_void, CString};
use std::{rc::Rc, sync::Mutex};

use pipewire::{
    prelude::ReadableDict,
    registry::{GlobalObject, Registry},
    Context, MainLoop,
};

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

static mut PLUGIN: Option<DB_output_t> = None;
static mut _plugin_ptr: *mut DB_output_t = std::ptr::null_mut();

static mut DEADBEEF: Option<DeadBeef> = None;
static mut DEADBEEF_THREAD_ID: Option<std::thread::ThreadId> = None;

static state: Mutex<ddb_playback_state_e> = Mutex::new(DDB_PLAYBACK_STATE_STOPPED);

pub extern "C" fn init() -> c_int {
    *state.lock().unwrap() = DDB_PLAYBACK_STATE_STOPPED;

    0
}

pub extern "C" fn free() -> c_int {
    *state.lock().unwrap() = DDB_PLAYBACK_STATE_STOPPED;
    0
}

pub extern "C" fn setformat(_fmt: *mut ddb_waveformat_t) -> c_int {
    0
}

pub extern "C" fn play() -> c_int {
    *state.lock().unwrap() = DDB_PLAYBACK_STATE_PLAYING;
    0
}

pub extern "C" fn stop() -> c_int {
    *state.lock().unwrap() = DDB_PLAYBACK_STATE_STOPPED;
    0
}

pub extern "C" fn pause() -> c_int {
    *state.lock().unwrap() = DDB_PLAYBACK_STATE_PAUSED;
    0
}

pub extern "C" fn unpause() -> c_int {
    *state.lock().unwrap() = DDB_PLAYBACK_STATE_PLAYING;
    0
}

pub extern "C" fn getstate() -> ddb_playback_state_t {
    *state.lock().unwrap()
}

pub extern "C" fn plugin_start() -> c_int {
    DeadBeef::log_detailed(DDB_LOG_LAYER_INFO, "Hello from rust!\n");
    0
}

pub extern "C" fn plugin_stop() -> c_int {
    unsafe {
        PLUGIN = None;
    }
    0
}

pub extern "C" fn enum_soundcards(
    callback: Option<
        unsafe extern "C" fn(name: *const c_char, desc: *const c_char, _userdata: *mut c_void),
    >,
    userdata: *mut c_void,
) {
    let mainloop = Rc::new(MainLoop::new().expect("Failed to create mainloop"));
    let context = Context::new(mainloop.as_ref()).expect("Failed to create context");
    let core = context.connect(None).expect("Failed to connect to remote");
    let registry = core.get_registry().expect("Failed to get registry");

    // Register a callback to the `global` event on the registry, which notifies of any new global objects
    // appearing on the remote.
    // The callback will only get called as long as we keep the returned listener alive.
    let _listener = registry
        .add_listener_local()
        .global(move |global| {
            if let Some(props) = &global.props {
                let media_class = props.get("media.class").unwrap_or("");
                if media_class.eq("Audio/Sink") || media_class.eq("Audio/Duplex") {
                    unsafe {
                        let n = LossyCString::new(props.get("node.name").unwrap_or(""));
                        let d = LossyCString::new(props.get("node.description").unwrap_or(""));

                        if n.as_bytes().len() > 0 {
                            callback.unwrap()(n.as_ptr(), d.as_ptr(), userdata);
                        }
                    }
                }
            }
        })
        .register();

    let _ml = Rc::downgrade(&mainloop);

    let _corelistener = core
        .add_listener_local()
        .done(move |id, seq| _ml.upgrade().unwrap().quit())
        .register();

    core.sync(0).expect("Error sync");

    mainloop.run();
}

pub unsafe extern "C" fn message(msgid: u32, ctx: usize, p1: u32, p2: u32) -> c_int {
    match msgid {
        DB_EV_SONGSTARTED => println!("rust: song started"),
        _ => return 0,
    }

    0
}

/// Main DeadBeef struct that encapsulates common DeadBeef API functions.
pub struct DeadBeef {
    pub(crate) ptr: *mut DB_functions_t,
}

impl DeadBeef {
    pub unsafe fn init_from_ptr(ptr: *mut DB_functions_t) -> DeadBeef {
        assert!(!ptr.is_null());

        DEADBEEF = Some(DeadBeef { ptr });
        DEADBEEF_THREAD_ID = Some(std::thread::current().id());

        DeadBeef { ptr }
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
            log_detailed(_plugin_ptr as *mut DB_plugin_t, layers, msg.as_ptr());
        }
    }

    pub fn streamer_read(buf: &mut Vec<i8>) -> i32 {
        let deadbeef = unsafe { DeadBeef::deadbeef() };

        let streamer_read = deadbeef.get().streamer_read.unwrap();

        unsafe { streamer_read(buf.as_mut_ptr(), buf.capacity().try_into().unwrap()) }
    }
}

pub(crate) struct LossyCString;

impl LossyCString {
    #[allow(clippy::new_ret_no_self)]
    pub(crate) fn new<T: AsRef<str>>(t: T) -> CString {
        match CString::new(t.as_ref()) {
            Ok(cstr) => cstr,
            Err(_) => CString::new(t.as_ref().replace('\0', "")).expect("string has no nulls"),
        }
    }
}

static plugin_id: &'static str = "pipewirerust\0";
static plugin_name: &'static str = "Pipewire output plugin written in Rust\0";
static plugin_desc: &'static str = "This is a new Pipewire based plugin written in rust\0";
static plugin_copyright: &'static str = "Some copyright\0";
static plugin_website: &'static str = "http://saivert.com\0";

macro_rules! lit_cstr {
    ($s:literal) => {
        (concat!($s, "\0").as_bytes().as_ptr() as *const c_char)
    };
}

#[no_mangle]
pub unsafe extern "C" fn libdeadbeef_rust_plugin_load(
    api: *mut DB_functions_t,
) -> *mut DB_output_s {

    DEADBEEF = Some(DeadBeef::init_from_ptr(api));

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
        has_volume: 0,

        fmt: ddb_waveformat_t::default(),

        plugin: DB_plugin_t {
            api_vmajor: 1,
            api_vminor: 0,
            version_major: 0,
            version_minor: 1,
            flags: DDB_PLUGIN_FLAG_LOGGING,
            type_: DB_PLUGIN_OUTPUT as i32,
            id: plugin_id.as_ptr() as *const c_char,
            name: plugin_name.as_ptr() as *const c_char,
            descr: plugin_desc.as_ptr() as *const c_char,
            copyright: plugin_copyright.as_ptr() as *const c_char,
            website: const_str::raw_cstr!("www.saivert.com"),
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

    PLUGIN = Some(x);

    _plugin_ptr = PLUGIN.as_mut().unwrap();

    _plugin_ptr
}
