use crate::*;

use std::thread;
use std::rc::Rc;

use pipewire::{prelude::*, properties, stream, Context, MainLoop, PW_ID_CORE};

pub struct OutputPlugin {
    plugin: DB_output_t,

    state: ddb_playback_state_e,
    thread: Option<PlaybackThread>,

    requested_fmt: Option<ddb_waveformat_t>,
}

struct PlaybackThread {
    handle: thread::JoinHandle<()>,
    sender: pipewire::channel::Sender<PwThreadMessage>,
}

#[derive(Debug)]
enum PwThreadMessage {
    Terminate,
    Pause,
    Unpause,
    SetFmt {
        format: u32,
        channels: u32,
        rate: u32,
    },
    SetVol {
        newvol: f32,
    },
}

impl PlaybackThread {
    pub fn new(init_fmt: ddb_waveformat_t) -> Self {
        let (sender, receiver) = pipewire::channel::channel();
        Self {
            handle: thread::spawn(move || pw_thread_main(init_fmt, receiver)),
            sender,
        }
    }

    pub fn join(self) -> thread::Result<()> {
        self.handle.join()
    }

    pub fn msg(&self, msg: PwThreadMessage) {
        self.sender
            .send(msg)
            .expect("Unable to send message to thread!")
    }
}

impl DBPlugin for OutputPlugin {

    fn new(plugin: DB_output_t) -> Self {
        Self {
            plugin,
            state: DDB_PLAYBACK_STATE_STOPPED,
            thread: None,
            requested_fmt: None,
        }
    }
    fn get_plugin_ptr(&mut self) -> *mut c_void {
        &mut self.plugin as *mut DB_output_t as *mut c_void
    }

    fn plugin_start(&mut self) {
        pipewire::init();
    }
    fn plugin_stop(&mut self) {
        unsafe { pipewire::deinit(); }
    }
}

impl OutputPlugin {
    fn msgtothread(&self, msg: PwThreadMessage) {
        if let Some(s) = self.thread.as_ref() {
            s.msg(msg);
        }
    }
}

impl DBOutput for OutputPlugin {

    fn init(&mut self) -> i32 {
        if let Some(rfmt) = self.requested_fmt {
            self.plugin.fmt = rfmt;
        } else {
            self.plugin.fmt = ddb_waveformat_t {
                samplerate: 44100,
                bps: 16,
                channels: 2,
                channelmask: 3,
                is_bigendian: 0,
                is_float: 0,
            }
        }

        self.thread = Some(PlaybackThread::new(self.plugin.fmt));

        self.state = DDB_PLAYBACK_STATE_STOPPED;
        0
    }

    fn play(&mut self) {
        if self.thread.is_none() {
            self.init();
        }
        self.state = DDB_PLAYBACK_STATE_PLAYING;
    }

    fn stop(&mut self) {
        self.msgtothread(PwThreadMessage::Terminate);
        if let Some(t) = self.thread.take() {
            match t.join() {
                Ok(_) => (),
                Err(_) => {
                    DeadBeef::log_detailed(DDB_LOG_LAYER_INFO, "Playback thread lingering!");
                }
            }
        }
        self.state = DDB_PLAYBACK_STATE_STOPPED;
        self.requested_fmt = None;
        self.thread = None;
    }

    fn free(&mut self) {
        self.stop();
    }

    fn pause(&mut self) {
        if self.thread.is_none() {
            self.init();
        }

        self.msgtothread(PwThreadMessage::Pause);
        self.state = DDB_PLAYBACK_STATE_PAUSED;
    }

    fn unpause(&mut self) {
        if self.thread.is_none() {
            self.init();
        }
        if self.state == DDB_PLAYBACK_STATE_PAUSED {
            self.msgtothread(PwThreadMessage::Unpause);
            self.state = DDB_PLAYBACK_STATE_PLAYING;
        }
    }

    fn getstate(&self) -> ddb_playback_state_e {
        self.state
    }

    fn setformat(&mut self, fmt: ddb_waveformat_t) {
        if fmt == self.plugin.fmt {
            println!("Format is equal. Not requesting change.");
            return;
        }
        self.requested_fmt = Some(fmt);
        self.plugin.fmt = fmt;
        print_db_format(fmt);
        let pwfmt = db_format_to_pipewire(fmt);
        self.msgtothread(PwThreadMessage::SetFmt {
            format: pwfmt,
            channels: fmt.channels as u32,
            rate: fmt.samplerate as u32,
        });
    }

    #[allow(unused)]
    fn message(&mut self, msgid: u32, ctx: usize, p1: u32, p2: u32) {
        match msgid {
            DB_EV_SONGSTARTED => println!("rust: song started"),
            DB_EV_VOLUMECHANGED => {
                self.msgtothread(PwThreadMessage::SetVol { newvol: DeadBeef::volume_get_amp() })
            },
            _ => {}
        }
    }

    fn enum_soundcards<F>(&self, callback: F)
    where F: Fn(&str, &str) + 'static  {
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
                        let name = props.get("node.name").unwrap_or("");
                        if !name.is_empty() {
                            callback(name, props.get("node.description").unwrap_or(""));
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
}

fn create_audio_format_pod(format: u32, channels: u32, rate: u32, buffer: &mut [u8]) -> *mut libspa_sys::spa_pod {
    unsafe {
        let mut b: libspa_sys::spa_pod_builder = std::mem::zeroed();
        b.data = buffer.as_mut_ptr() as *mut c_void;
        b.size = buffer.len() as u32;
        let mut audioinfo = libspa_sys::spa_audio_info_raw {
            format,
            flags: 0,
            rate,
            channels,
            position: std::mem::zeroed(),
        };
        libspa_sys::spa_format_audio_raw_build(&mut b as *mut libspa_sys::spa_pod_builder,
            libspa_sys::SPA_PARAM_EnumFormat,
            &mut audioinfo as *mut libspa_sys::spa_audio_info_raw)
    }
}

fn pw_thread_main(init_fmt: ddb_waveformat_t, pw_receiver: pipewire::channel::Receiver<PwThreadMessage>) {
    let mainloop = MainLoop::new().expect("Failed to create mainloop");

    let device = DeadBeef::conf_get_str("pipewirerust_soundcard", "default");

    let mut props = properties! {
        *pipewire::keys::MEDIA_TYPE => "Audio",
        *pipewire::keys::MEDIA_TYPE => "Audio",
        *pipewire::keys::MEDIA_CATEGORY => "Playback",
        *pipewire::keys::MEDIA_ROLE => "Music",
        *pipewire::keys::NODE_NAME => "DeadBeef",
        *pipewire::keys::APP_NAME => "DeadBeef",
        *pipewire::keys::APP_ID => "music.player.deadbeef",
    };

    let s = format!("1/{}", init_fmt.samplerate);
    props.insert("node.rate", &s);

    if !device.eq("default") {
        props.insert(*pipewire::keys::NODE_TARGET, &device);
    }

    let ourdisconnect = Rc::new(std::cell::Cell::new(false));

    let stream = pipewire::stream::Stream::<i32>::with_user_data(
        &mainloop,
        "deadbeef",
        props,
        0,
    )
    .state_changed({
        let ourdisconnect = ourdisconnect.clone();
        move |old, new| {
            println!("State changed: {:?} -> {:?}", old, new);
            match new {
                pipewire::stream::StreamState::Error(x) => {
                    let msg = format!("Pipewire playback error: {x}");
                    DeadBeef::log_detailed(DDB_LOG_LAYER_DEFAULT, &msg);
                    DeadBeef::sendmessage(DB_EV_STOP, 0, 0, 0);
                },
                pipewire::stream::StreamState::Unconnected => {
                    if !ourdisconnect.get() {
                        DeadBeef::log_detailed(DDB_LOG_LAYER_DEFAULT, "Pipewire disconnected.");
                        DeadBeef::sendmessage(DB_EV_STOP, 0, 0, 0);
                    }
                },
                pipewire::stream::StreamState::Connecting => {
                    ourdisconnect.set(false);
                }
                _ => {}
            }
        }
    })
    .process(move |stream, _user_data| {
        match stream.dequeue_buffer() {
            None => println!("No buffer received"),
            Some(mut buffer) => {
                let datas = buffer.datas_mut();
                if let Some(d) = datas[0].data() {

                    let bytesread = if DeadBeef::streamer_ok_to_read(-1) > 0 {
                        DeadBeef::streamer_read(d.as_mut_ptr() as *mut c_void, 4096)
                    } else {
                        stream.flush(false).expect("");
                        0
                    };

                    *datas[0].chunk_mut().size_mut() = bytesread as u32;
                    *datas[0].chunk_mut().offset_mut() = 0;
                    *datas[0].chunk_mut().stride_mut() = 1;
                }
            }
        };
    })
    .control_info(|id, control_ptr: *const pipewire::sys::pw_stream_control| {
        if id == libspa_sys::SPA_PROP_channelVolumes {
            unsafe {
                let control = *control_ptr;
                let values = std::slice::from_raw_parts(control.values, control.n_values as usize);
                DeadBeef::volume_set_amp(values[0]);
            }
        }
    })
    .create()
    .expect("Error creating stream!");

    let mut buffer = [0;1024];
    let fmtpod = {
        let fmt = init_fmt;
        let format = db_format_to_pipewire(fmt);
        let channels = fmt.channels as u32;
        let rate = fmt.samplerate as u32;

        create_audio_format_pod(format, channels, rate, &mut buffer)
    };

    stream
        .connect(
            pipewire::spa::Direction::Output,
            None,
            stream::StreamFlags::AUTOCONNECT
                | stream::StreamFlags::MAP_BUFFERS
                | stream::StreamFlags::RT_PROCESS,
            &mut [fmtpod],
        )
        .expect("Error connecting stream!");

    // When we receive a `Terminate` message, quit the main loop.
    let _receiver = pw_receiver.attach(&mainloop, {
        let mainloop = mainloop.clone();
        move |msg| {
            match msg {
                PwThreadMessage::Terminate => mainloop.quit(),
                PwThreadMessage::Pause => stream.set_active(false).unwrap(),
                PwThreadMessage::Unpause => stream.set_active(true).unwrap(),
                PwThreadMessage::SetFmt { format, channels, rate } => {
                    ourdisconnect.set(true);
                    if stream.disconnect().is_ok() {

                        println!("Set format called with: Format = {format}, Channels = {channels}, rate = {rate}");
                        print_pipewire_format(format, channels, rate);
    
                        let mut buffer = [0;1024];
                        let newformatpod: *mut libspa_sys::spa_pod = create_audio_format_pod(format, channels, rate, &mut buffer);

                        stream
                        .connect(
                            pipewire::spa::Direction::Output,
                            None,
                            stream::StreamFlags::AUTOCONNECT
                                | stream::StreamFlags::MAP_BUFFERS
                                | stream::StreamFlags::RT_PROCESS,
                            &mut [newformatpod],
                        )
                        .expect("Error connecting stream!");

                        let s = format!("1/{}", rate);
                        let props = properties!{
                            "node.rate" => s
                        };
                        unsafe {
                            pipewire::sys::pw_stream_update_properties(stream.as_ptr(), props.get_dict_ptr());
                        }
                    }
                },
                PwThreadMessage::SetVol { newvol } => {
                    let values = [newvol, newvol];
                    stream.set_control(libspa_sys::SPA_PROP_channelVolumes, &values).expect("Unable to set volume");
                }
            };
        }
    });

    mainloop.run();

}
