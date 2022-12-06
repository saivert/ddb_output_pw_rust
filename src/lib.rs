#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![deny(elided_lifetimes_in_paths)]
extern crate libspa_sys;

use cpp::cpp;

cpp!{{
    #include <spa/param/audio/format-utils.h>
    #include <spa/param/props.h>
    #include <pipewire/pipewire.h>

}}

use std::ffi::{c_char, c_int, c_void, CString};

use std::{rc::Rc, sync::Mutex};
use std::thread;

use pipewire::{
    prelude::*,
    properties,
    Context, MainLoop, stream,
};


include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

static mut PLUGIN: Option<DB_output_t> = None;
static mut _plugin_ptr: *mut DB_output_t = std::ptr::null_mut();

static mut DEADBEEF: Option<DeadBeef> = None;
static mut DEADBEEF_THREAD_ID: Option<std::thread::ThreadId> = None;

static state: Mutex<ddb_playback_state_e> = Mutex::new(DDB_PLAYBACK_STATE_STOPPED);

#[derive(Debug)]
pub enum PwThreadMessage {
    Terminate,
    Pause,
    Unpause
}

static RESULT_SENDER: Mutex<Option<pipewire::channel::Sender<PwThreadMessage>>> = Mutex::new(None);

pub fn pw_thread_main(pw_receiver: pipewire::channel::Receiver<PwThreadMessage>) {

    let mainloop = MainLoop::new().expect("Failed to create mainloop");



    let stream = pipewire::stream::Stream::<i32>::with_user_data(
        &mainloop,
        "deadbeef",
        properties! {
            *pipewire::keys::MEDIA_TYPE => "Audio",
            *pipewire::keys::MEDIA_CATEGORY => "Playback",
            *pipewire::keys::MEDIA_ROLE => "Music",
            *pipewire::keys::NODE_NAME => "DeadBeef [rust]",
            *pipewire::keys::APP_NAME => "DeadBeef [rust]",
            *pipewire::keys::APP_ID => "music.player.deadbeef",
        },0
    )
    .state_changed(|old, new| {
        println!("State changed: {:?} -> {:?}", old, new);
    })
    .process(move |stream, _user_data| {
        println!("On frame");
        match stream.dequeue_buffer() {
            None => println!("No buffer received"),
            Some(mut buffer) => {
                let datas = buffer.datas_mut();
                println!("Frame {}. Got {} datas.", _user_data, datas.len());
                *_user_data += 1;

                let d = datas[0].get_mut();

                let bytesread = if DeadBeef::streamer_ok_to_read(-1) > 0 {
                    DeadBeef::streamer_read(d.as_mut_ptr() as *mut c_void, 4096)
                } else {
                    0
                }; 

                *datas[0].chunk().size_mut() = bytesread as u32;
                *datas[0].chunk().offset_mut() = 0;
                *datas[0].chunk().stride_mut() = 1;
            }
        };

    })
    .create().expect("Error creating stream!");

    // Until pipewire-rs get bindings for POD building we have to cheat and use C++ for this
    let r: *mut libspa_sys::spa_pod = unsafe {
        cpp!([] -> *mut libspa_sys::spa_pod as "spa_pod *" {
            uint8_t buffer[1024];

            struct spa_pod_builder b = SPA_POD_BUILDER_INIT(buffer, sizeof(buffer));
        
            struct spa_audio_info_raw rawinfo = {};
            rawinfo.format =  SPA_AUDIO_FORMAT_S16_LE;
            rawinfo.channels = 2;
            rawinfo.rate = 48000;
            return spa_format_audio_raw_build(&b, SPA_PARAM_EnumFormat, &rawinfo);
        })
    };

    stream.connect(pipewire::spa::Direction::Output, None, 
        stream::StreamFlags::AUTOCONNECT|stream::StreamFlags::MAP_BUFFERS|stream::StreamFlags::RT_PROCESS,
        &mut [r],
    ).expect("Error connecting stream!");

    *state.lock().unwrap() = DDB_PLAYBACK_STATE_PLAYING;

     // When we receive a `Terminate` message, quit the main loop.
     let _receiver = pw_receiver.attach(&mainloop, {
        let mainloop = mainloop.clone();
        move |msg| {
           match msg {
               PwThreadMessage::Terminate => mainloop.quit(),
               PwThreadMessage::Pause => stream.set_active(false).unwrap(),
               PwThreadMessage::Unpause => stream.set_active(true).unwrap(),
           };
        }
    });

    mainloop.run();

    *RESULT_SENDER.lock().unwrap() = None;

}


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

    let (pw_sender, pw_receiver) = pipewire::channel::channel();

    *RESULT_SENDER.lock().unwrap() = Some(pw_sender);

    let _pw_thread =
    thread::spawn(||pw_thread_main(pw_receiver));

    *state.lock().unwrap() = DDB_PLAYBACK_STATE_PLAYING;

    0
}

fn msgtopwthread(msg: PwThreadMessage) {
    if let Ok(sendermtx) = RESULT_SENDER.lock() {
        if let Some(sender) = sendermtx.as_ref() {
            sender.send(msg).expect("Cannot send message!");
        }
    }
}

pub extern "C" fn stop() -> c_int {
    msgtopwthread(PwThreadMessage::Terminate);
    *state.lock().unwrap() = DDB_PLAYBACK_STATE_STOPPED;
    0
}

pub extern "C" fn pause() -> c_int {
    msgtopwthread(PwThreadMessage::Pause);
    *state.lock().unwrap() = DDB_PLAYBACK_STATE_PAUSED;
    0
}

pub extern "C" fn unpause() -> c_int {
    msgtopwthread(PwThreadMessage::Unpause);
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

    let _corelistener = core
        .add_listener_local()
        .done({
            let mainloop = mainloop.clone();
            move |_id, _seq| mainloop.quit()
        })
        .register();

    core.sync(0).expect("Error sync");

    mainloop.run();
}

#[allow(unused)]
pub unsafe extern "C" fn message(msgid: u32, ctx: usize, p1: u32, p2: u32) -> c_int {
    match msgid {
        DB_EV_SONGSTARTED => println!("rust: song started"),
        _ => return 0,
    }

    0
}

/// Main DeadBeef struct that encapsulates common DeadBeef API functions.
pub struct DeadBeef {
    pub(crate) ptr: *const DB_functions_t,
}

impl DeadBeef {
    pub unsafe fn init_from_ptr(ptr: *const DB_functions_t) -> DeadBeef {
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
    api: *const DB_functions_t,
) -> *mut DB_output_s {

    DEADBEEF = Some(DeadBeef::init_from_ptr(api));

    let mut x = DB_output_t {
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

    // We only handle this format for now
    x.fmt.channels = 2;
    x.fmt.bps = 16;
    x.fmt.is_float = 0;
    x.fmt.is_bigendian = 0;
    x.fmt.samplerate = 48000;
    x.fmt.channelmask = 3;

    PLUGIN = Some(x);

    _plugin_ptr = PLUGIN.as_mut().unwrap();

    _plugin_ptr
}
