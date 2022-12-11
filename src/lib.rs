use cpp::cpp;

cpp! {{
    #include <spa/param/audio/format-utils.h>
    #include <spa/param/props.h>
    #include <pipewire/pipewire.h>

}}

use std::ffi::{c_char, c_int, c_void};

use std::thread;
use std::{rc::Rc, sync::Mutex};

use pipewire::{prelude::*, properties, stream, Context, MainLoop, PW_ID_CORE};

mod dbapi;
use dbapi::*;

#[macro_use]
mod utils;
use utils::LossyCString;

static mut PLUGIN: Option<DB_output_t> = None;

static mut DEADBEEF: Option<DeadBeef> = None;
static mut DEADBEEF_THREAD_ID: Option<std::thread::ThreadId> = None;

static STATE: Mutex<ddb_playback_state_e> = Mutex::new(DDB_PLAYBACK_STATE_STOPPED);

#[derive(Debug)]
pub enum PwThreadMessage {
    Terminate,
    Pause,
    Unpause,
    SetFmt{format: u32, channels: u32, rate: u32},
}

static RESULT_SENDER: Mutex<Option<pipewire::channel::Sender<PwThreadMessage>>> = Mutex::new(None);

pub fn db_format_to_pipewire(input: ddb_waveformat_t) -> u32 {
    match input.bps {
        8 => libspa_sys::spa_audio_format_SPA_AUDIO_FORMAT_S8,
        16 => libspa_sys::spa_audio_format_SPA_AUDIO_FORMAT_S16_LE,
        24 => libspa_sys::spa_audio_format_SPA_AUDIO_FORMAT_S24_LE,
        32 => match input.is_float == 1 {
            true => libspa_sys::spa_audio_format_SPA_AUDIO_FORMAT_F32_LE,
            false => libspa_sys::spa_audio_format_SPA_AUDIO_FORMAT_S32_LE,
        },
        _ => libspa_sys::spa_audio_format_SPA_AUDIO_FORMAT_UNKNOWN,
    }
}

pub fn pw_thread_main(pw_receiver: pipewire::channel::Receiver<PwThreadMessage>) {
    let mainloop = MainLoop::new().expect("Failed to create mainloop");

    let device = DeadBeef::conf_get_str("pipewirerust_soundcard", "default");

    let mut props = properties! {
        *pipewire::keys::MEDIA_TYPE => "Audio",
        *pipewire::keys::MEDIA_TYPE => "Audio",
        *pipewire::keys::MEDIA_CATEGORY => "Playback",
        *pipewire::keys::MEDIA_ROLE => "Music",
        *pipewire::keys::NODE_NAME => "DeadBeef [rust]",
        *pipewire::keys::APP_NAME => "DeadBeef [rust]",
        *pipewire::keys::APP_ID => "music.player.deadbeef",
        "node.rate" => "1/48000",
    };

    if !device.eq("default") {
        props.insert(*pipewire::keys::NODE_TARGET, &device);
    }

    let stream = pipewire::stream::Stream::<i32>::with_user_data(
        &mainloop,
        "deadbeef",
        props,
        0,
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
    .create()
    .expect("Error creating stream!");

    // Until pipewire-rs get bindings for POD building we have to cheat and use C++ for this
    let r: *mut libspa_sys::spa_pod = unsafe {
        let fmt = PLUGIN.unwrap().fmt;
        let format = db_format_to_pipewire(fmt);
        let channels = fmt.channels;
        let rate = fmt.samplerate;
        cpp!([format as "int", channels as "int", rate as "int"] -> *mut libspa_sys::spa_pod as "spa_pod *" {
            uint8_t buffer[1024];

            struct spa_pod_builder b = SPA_POD_BUILDER_INIT(buffer, sizeof(buffer));

            struct spa_audio_info_raw rawinfo = {};
            rawinfo.format = (enum spa_audio_format) format;
            rawinfo.channels = channels;
            rawinfo.rate = rate;
            return spa_format_audio_raw_build(&b, SPA_PARAM_EnumFormat, &rawinfo);
        })
    };

    stream
        .connect(
            pipewire::spa::Direction::Output,
            None,
            stream::StreamFlags::AUTOCONNECT
                | stream::StreamFlags::MAP_BUFFERS
                | stream::StreamFlags::RT_PROCESS,
            &mut [r],
        )
        .expect("Error connecting stream!");

    *STATE.lock().unwrap() = DDB_PLAYBACK_STATE_PLAYING;

    // When we receive a `Terminate` message, quit the main loop.
    let _receiver = pw_receiver.attach(&mainloop, {
        let mainloop = mainloop.clone();
        move |msg| {
            match msg {
                PwThreadMessage::Terminate => mainloop.quit(),
                PwThreadMessage::Pause => stream.set_active(false).unwrap(),
                PwThreadMessage::Unpause => stream.set_active(true).unwrap(),
                PwThreadMessage::SetFmt { format, channels, rate } => {
                    if stream.disconnect().is_ok() {

                        println!("Set format called with: Format = {format}, Channels = {channels}, rate = {rate}");

                        let new_format: *mut libspa_sys::spa_pod = unsafe {
                            cpp!([format as "int", channels as "int", rate as "int"] -> *mut libspa_sys::spa_pod as "spa_pod *" {
                                uint8_t buffer[1024];

                                struct spa_pod_builder b = SPA_POD_BUILDER_INIT(buffer, sizeof(buffer));
                    
                                struct spa_audio_info_raw rawinfo = {};
                                rawinfo.format = (enum spa_audio_format)format;
                                rawinfo.channels = channels;
                                rawinfo.rate = rate;
                                return spa_format_audio_raw_build(&b, SPA_PARAM_EnumFormat, &rawinfo);
                            })
                        };

                        stream
                        .connect(
                            pipewire::spa::Direction::Output,
                            None,
                            stream::StreamFlags::AUTOCONNECT
                                | stream::StreamFlags::MAP_BUFFERS
                                | stream::StreamFlags::RT_PROCESS,
                            &mut [new_format],
                        )
                        .expect("Error connecting stream!");


                    }
                }
            };
        }
    });

    mainloop.run();

    *RESULT_SENDER.lock().unwrap() = None;
}

pub extern "C" fn init() -> c_int {
    *STATE.lock().unwrap() = DDB_PLAYBACK_STATE_STOPPED;

    0
}

pub extern "C" fn free() -> c_int {
    *STATE.lock().unwrap() = DDB_PLAYBACK_STATE_STOPPED;
    0
}

pub extern "C" fn setformat(fmt: *mut ddb_waveformat_t) -> c_int {
    /* Not working right yet... causes garbled audio and crashes sometimes.
    unsafe {
        let pwfmt = db_format_to_pipewire(*fmt);
        PLUGIN.unwrap().fmt = *fmt;
        msgtopwthread(PwThreadMessage::SetFmt { format: pwfmt, channels: (*fmt).channels as u32, rate: (*fmt).samplerate as u32 });
    }
    */
    0
}

pub extern "C" fn play() -> c_int {
    let (pw_sender, pw_receiver) = pipewire::channel::channel();

    *RESULT_SENDER.lock().unwrap() = Some(pw_sender);

    let _pw_thread = thread::spawn(|| pw_thread_main(pw_receiver));

    *STATE.lock().unwrap() = DDB_PLAYBACK_STATE_PLAYING;

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
    RESULT_SENDER.lock().unwrap().take();
    *STATE.lock().unwrap() = DDB_PLAYBACK_STATE_STOPPED;
    0
}

pub extern "C" fn pause() -> c_int {
    msgtopwthread(PwThreadMessage::Pause);
    *STATE.lock().unwrap() = DDB_PLAYBACK_STATE_PAUSED;
    0
}

pub extern "C" fn unpause() -> c_int {
    msgtopwthread(PwThreadMessage::Unpause);
    *STATE.lock().unwrap() = DDB_PLAYBACK_STATE_PLAYING;
    0
}

pub extern "C" fn getstate() -> ddb_playback_state_t {
    *STATE.lock().unwrap()
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
    let mainloop = MainLoop::new().expect("Failed to create mainloop");
    let context = Context::new(&mainloop).expect("Failed to create context");
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

                        if !n.as_bytes().is_empty() {
                            callback.unwrap()(n.as_ptr(), d.as_ptr(), userdata);
                        }
                    }
                }
            }
        })
        .register();

    let done = Rc::new(std::cell::Cell::new(false));
    let pending = core.sync(0).expect("Error sync");
    let mainloop_clone = mainloop.clone();
    let done_clone = done.clone();

    let _core = core
        .add_listener_local()
        .done(move |id, seq| {
            if id == PW_ID_CORE && seq == pending {
                done_clone.set(true);
                mainloop_clone.quit()
            }
        })
        .register();

    while !done.get() {
        mainloop.run();
    }
}

#[allow(unused)]
pub extern "C" fn message(msgid: u32, ctx: usize, p1: u32, p2: u32) -> c_int {
    match msgid {
        DB_EV_SONGSTARTED => println!("rust: song started"),
        _ => return 0,
    }

    0
}

#[no_mangle]
///
/// # Safety
/// This is requires since this is a plugin export function
pub unsafe extern "C" fn libdeadbeef_rust_plugin_load(
    api: *const DB_functions_t,
) -> *mut DB_output_s {
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
            id: lit_cstr!("pipewirerust"),
            name: lit_cstr!("Pipewire output plugin written in Rust"),
            descr: lit_cstr!("This is a new Pipewire based plugin written in rust"),
            copyright: lit_cstr!("Some copyright"),
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

    // We only handle this format for now
    x.fmt.channels = 2;
    x.fmt.bps = 16;
    x.fmt.is_float = 0;
    x.fmt.is_bigendian = 0;
    x.fmt.samplerate = 48000;
    x.fmt.channelmask = 3;

    PLUGIN = Some(x);

    let plugin_ptr: *mut DB_output_t = PLUGIN.as_mut().unwrap();

    DEADBEEF = Some(DeadBeef::init_from_ptr(api, plugin_ptr));

    plugin_ptr
}
