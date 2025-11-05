use crate::*;

use std::rc::Rc;
use std::{cell::Cell, thread};

use pipewire::{
    context::Context,
    core::PW_ID_CORE,
    main_loop::MainLoop,
    properties::properties,
    spa::utils::Direction,
    stream::{self, StreamFlags},
};

pub struct OutputPlugin {
    plugin: DB_output_t,

    state: PlaybackState,
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
        format: ddb_waveformat_t,
        state: PlaybackState,
    },
    SetVol {
        newvol: f32,
    },
    SetTitle(String),
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
    fn get_plugin_ptr(&self) -> *const DB_output_t {
        &self.plugin as *const DB_output_t
    }
}

impl OutputPlugin {
    pub fn new(plugin: DB_output_t) -> Self {
        Self {
            plugin,
            state: PlaybackState::Stopped,
            thread: None,
            requested_fmt: None,
        }
    }

    pub fn plugin_start(&mut self) {
        pipewire::init();
    }
    pub fn plugin_stop(&mut self) {
        unsafe {
            pipewire::deinit();
        }
    }

    #[allow(unused)]
    pub fn message(&mut self, msgid: u32, ctx: usize, p1: u32, p2: u32) {
        match msgid {
            DB_EV_VOLUMECHANGED => self.msgtothread(PwThreadMessage::SetVol {
                newvol: DeadBeef::volume_get_amp(),
            }),
            DB_EV_SONGCHANGED => {
                if let Ok(media_name) = DeadBeef::titleformat("[%artist% - ]%title%") {
                    self.msgtothread(PwThreadMessage::SetTitle(media_name))
                }
            }
            _ => {}
        }
    }

    fn msgtothread(&self, msg: PwThreadMessage) {
        if let Some(s) = self.thread.as_ref() {
            s.msg(msg);
        }
    }

    pub fn init(&mut self) -> i32 {
        if self.requested_fmt.is_none() {
            self.requested_fmt = Some(get_default_waveformat());
        }

        self.plugin.fmt = self.requested_fmt.unwrap();

        self.thread = Some(PlaybackThread::new(self.plugin.fmt));

        self.state = PlaybackState::Stopped;
        0
    }

    pub fn play(&mut self) {
        if self.thread.is_none() {
            self.init();
        }
        self.state = PlaybackState::Playing;
    }

    pub fn stop(&mut self) {
        self.msgtothread(PwThreadMessage::Terminate);
        if let Some(t) = self.thread.take() {
            match t.join() {
                Ok(_) => (),
                Err(_) => {
                    DeadBeef::log_detailed(DDB_LOG_LAYER_INFO, "Playback thread lingering!");
                }
            }
        }
        self.state = PlaybackState::Stopped;
        self.thread = None;
    }

    pub fn free(&mut self) {
        self.stop();
    }

    pub fn pause(&mut self) {
        if self.thread.is_none() {
            self.init();
        }

        self.msgtothread(PwThreadMessage::Pause);
        self.state = PlaybackState::Paused;
    }

    pub fn unpause(&mut self) {
        if self.thread.is_none() {
            self.init();
        }
        if self.state == PlaybackState::Paused {
            self.msgtothread(PwThreadMessage::Unpause);
            self.state = PlaybackState::Playing;
        }
    }

    pub fn getstate(&self) -> ddb_playback_state_e {
        self.state.as_raw()
    }

    pub fn setformat(&mut self, fmt: ddb_waveformat_t) {
        if fmt == self.plugin.fmt {
            debug!("Format is equal. Not requesting change.");
            return;
        }
        self.plugin.fmt = if fmt.channels == 0 {
            get_default_waveformat()
        } else {
            fmt
        };
        self.requested_fmt = Some(self.plugin.fmt);
        print_db_format(fmt);
        self.msgtothread(PwThreadMessage::SetFmt {
            format: fmt,
            state: self.state,
        });
    }

    pub fn enum_soundcards<F>(&self, callback: F)
    where
        F: Fn(&str, &str) + 'static,
    {
        let mainloop = MainLoop::new(None).expect("Failed to create mainloop");
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

fn make_channel_map(channels: u32) -> [u32; 64] {
    let mut position = [0; 64];
    if channels == 1 {
        position[0] = libspa_sys::SPA_AUDIO_CHANNEL_MONO;
    }
    if channels >= 2 {
        position[0] = libspa_sys::SPA_AUDIO_CHANNEL_FL;
        position[1] = libspa_sys::SPA_AUDIO_CHANNEL_FR;
    }
    if channels >= 3 {
        position[2] = libspa_sys::SPA_AUDIO_CHANNEL_FC;
    }
    if channels >= 4 {
        position[3] = libspa_sys::SPA_AUDIO_CHANNEL_LFE;
    }
    if channels >= 6 {
        position[4] = libspa_sys::SPA_AUDIO_CHANNEL_RL;
        position[5] = libspa_sys::SPA_AUDIO_CHANNEL_RR;
    }
    if channels >= 8 {
        position[6] = libspa_sys::SPA_AUDIO_CHANNEL_FLC;
        position[7] = libspa_sys::SPA_AUDIO_CHANNEL_FRC;
    }
    if channels >= 9 {
        position[8] = libspa_sys::SPA_AUDIO_CHANNEL_RC;
    }
    if channels >= 11 {
        position[9] = libspa_sys::SPA_AUDIO_CHANNEL_SL;
        position[10] = libspa_sys::SPA_AUDIO_CHANNEL_SR;
    }
    if channels >= 12 {
        position[11] = libspa_sys::SPA_AUDIO_CHANNEL_TC;
    }
    if channels >= 15 {
        position[12] = libspa_sys::SPA_AUDIO_CHANNEL_TFL;
        position[13] = libspa_sys::SPA_AUDIO_CHANNEL_TFC;
        position[14] = libspa_sys::SPA_AUDIO_CHANNEL_TFR;
    }
    if channels >= 18 {
        position[15] = libspa_sys::SPA_AUDIO_CHANNEL_TRL;
        position[16] = libspa_sys::SPA_AUDIO_CHANNEL_TRC;
        position[17] = libspa_sys::SPA_AUDIO_CHANNEL_TRR;
    }

    position
}

fn create_audio_format_pod(
    format: pipewire::spa::param::audio::AudioFormat,
    channels: u32,
    rate: u32,
    buffer: &mut Vec<u8>,
) -> &pipewire::spa::pod::Pod {
    let mut audio_info = pipewire::spa::param::audio::AudioInfoRaw::new();
    audio_info.set_format(format);
    audio_info.set_rate(rate);
    audio_info.set_channels(channels);

    audio_info.set_position(make_channel_map(channels));

    let values = pipewire::spa::pod::serialize::PodSerializer::serialize(
        std::io::Cursor::new(buffer),
        &pipewire::spa::pod::Value::Object(pipewire::spa::pod::Object {
            type_: libspa_sys::SPA_TYPE_OBJECT_Format,
            id: libspa_sys::SPA_PARAM_EnumFormat,
            properties: audio_info.into(),
        }),
    )
    .unwrap()
    .0
    .into_inner();

    pipewire::spa::pod::Pod::from_bytes(values).unwrap()
}

fn pw_thread_main(
    init_fmt: ddb_waveformat_t,
    pw_receiver: pipewire::channel::Receiver<PwThreadMessage>,
) {
    let mainloop = MainLoop::new(None).expect("Failed to create mainloop");
    let client_props = properties! {
        *pipewire::keys::APP_NAME => "DeadBeef",
        *pipewire::keys::APP_ID => "music.player.deadbeef",
        *pipewire::keys::APP_ICON_NAME => "deadbeef"
    };
    let context = Context::new(&mainloop).expect("Context");
    let core = context.connect(Some(client_props)).expect("Core");

    let device = DeadBeef::conf_get_str("pipewirerust_soundcard", "default");

    let mut props = properties! {
        *pipewire::keys::MEDIA_TYPE => "Audio",
        *pipewire::keys::MEDIA_CATEGORY => "Playback",
        *pipewire::keys::MEDIA_ROLE => "Music",
        *pipewire::keys::NODE_NAME => "DeadBeef",
        *pipewire::keys::APP_NAME => "DeadBeef",
        *pipewire::keys::APP_ID => "music.player.deadbeef",
        *pipewire::keys::APP_ICON_NAME => "deadbeef",
        "node.latency" => "1200/48000",
    };

    let s = format!("1/{}", init_fmt.samplerate);
    props.insert("node.rate", s);

    if !device.eq("default") {
        props.insert(*pipewire::keys::TARGET_OBJECT, device);
    }

    if let Ok(media_name) = DeadBeef::titleformat("[%artist% - ]%title%") {
        props.insert(*pipewire::keys::MEDIA_NAME, media_name);
    }

    let ourdisconnect = Rc::new(Cell::new(false));
    let fmt = Rc::new(Cell::new(init_fmt));

    let stream: stream::Stream = match pipewire::stream::Stream::new(&core, "deadbeef", props) {
        Ok(a) => a,
        Err(e) => {
            DeadBeef::log_detailed(
                DDB_LOG_LAYER_DEFAULT,
                format!("Pipewire: Unable to create stream, {}\n", e.to_string()).as_str(),
            );
            DeadBeef::sendmessage(DB_EV_STOP, 0, 0, 0);
            return;
        }
    };

    let _listener = stream
        .add_local_listener::<()>()
        .state_changed({
            let ourdisconnect = ourdisconnect.clone();
            move |_stream, _userdata, _old, new| {
                debug!("State changed: {_old:?} -> {new:?}");
                match new {
                    pipewire::stream::StreamState::Error(x) => {
                        let msg = format!("Pipewire playback error: {x}");
                        DeadBeef::log_detailed(DDB_LOG_LAYER_DEFAULT, &msg);
                        DeadBeef::sendmessage(DB_EV_STOP, 0, 0, 0);
                    }
                    pipewire::stream::StreamState::Unconnected => {
                        if !ourdisconnect.get() {
                            DeadBeef::log_detailed(DDB_LOG_LAYER_DEFAULT, "Pipewire disconnected.");
                            DeadBeef::sendmessage(DB_EV_STOP, 0, 0, 0);
                        }
                    }
                    pipewire::stream::StreamState::Connecting => {
                        ourdisconnect.set(false);
                    }
                    _ => {}
                }
            }
        })
        .process({
            let fmt = fmt.clone();
            let ourdisconnect = ourdisconnect.clone();
            move |stream, _userdata| {
                let fmt = fmt.get();

                // This prevents glitches during format changes
                if ourdisconnect.get() {
                    return;
                }

                match stream.dequeue_buffer() {
                    None => debug!("No buffer received"),
                    Some(mut buffer) => {
                        let req = buffer.requested();
                        let datas = buffer.datas_mut();

                        let maxsize = datas[0].as_raw().maxsize as i32;
                        if let Some(d) = datas[0].data() {
                            let stride = fmt.channels * (fmt.bps / 8);

                            let len = if req > 0 {
                                req as i32 * stride
                            } else {
                                let buffersize = 25 * fmt.samplerate / 1000;
                                buffersize.min(maxsize / stride)
                            };

                            let bytesread = if DeadBeef::streamer_ok_to_read(-1) > 0 {
                                DeadBeef::streamer_read(d.as_mut_ptr() as *mut c_void, len as usize)
                            } else {
                                0
                            };

                            if bytesread < len {
                                d[bytesread as usize..].fill(0);
                            }

                            *datas[0].chunk_mut().size_mut() = bytesread as u32;
                            *datas[0].chunk_mut().offset_mut() = 0;
                            *datas[0].chunk_mut().stride_mut() = stride;
                        }
                    }
                };
            }
        })
        .control_info(
            move |_stream, _userdata, id, control_ptr: *const pipewire::sys::pw_stream_control| {
                if id == libspa_sys::SPA_PROP_channelVolumes {
                    unsafe {
                        let control = *control_ptr;
                        if control.n_values > 0 {
                            let values = std::slice::from_raw_parts(
                                control.values,
                                control.n_values as usize,
                            );
                            for v in values.iter() {
                                if *v != DeadBeef::volume_get_amp() {
                                    DeadBeef::volume_set_amp(*v);
                                    break;
                                }
                            }
                        }
                    }
                }
            },
        )
        .register();

    let mut buffer: Vec<u8> = Vec::new();
    let fmtpod = {
        let fmt = init_fmt;
        let format = db_format_to_pipewire(fmt);
        let channels = fmt.channels as u32;
        let rate = fmt.samplerate as u32;

        create_audio_format_pod(format, channels, rate, &mut buffer)
    };

    if let Err(e) = stream.connect(
        Direction::Output,
        None,
        StreamFlags::AUTOCONNECT | StreamFlags::MAP_BUFFERS | StreamFlags::RT_PROCESS,
        &mut [&fmtpod],
    ) {
        DeadBeef::log_detailed(
            DDB_LOG_LAYER_DEFAULT,
            format!("Pipewire: Unable to connect stream, {}\n", e.to_string()).as_str(),
        );
        DeadBeef::sendmessage(DB_EV_STOP, 0, 0, 0);
        return;
    }

    // When we receive a `Terminate` message, quit the main loop.
    let _receiver = pw_receiver.attach(mainloop.as_ref(), {
        let mainloop = mainloop.clone();
        move |msg| {
            match msg {
                PwThreadMessage::Terminate => {
                    ourdisconnect.set(true);
                    mainloop.quit();
                }
                PwThreadMessage::Pause => stream.set_active(false).unwrap(),
                PwThreadMessage::Unpause => stream.set_active(true).unwrap(),
                PwThreadMessage::SetFmt { format, state } => {
                    ourdisconnect.set(true);
                    if stream.disconnect().is_ok() {
                        debug!("Set format called with: ");
                        let pwfmt = db_format_to_pipewire(format);
                        let channels = format.channels as u32;
                        let samplerate = format.samplerate as u32;
                        print_pipewire_format(pwfmt, channels, samplerate);

                        let mut buffer: Vec<u8> = Vec::new();
                        let newformatpod =
                            create_audio_format_pod(pwfmt, channels, samplerate, &mut buffer);
                        fmt.set(format);

                        let mut flags = StreamFlags::AUTOCONNECT
                            | StreamFlags::MAP_BUFFERS
                            | StreamFlags::RT_PROCESS;

                        if state != PlaybackState::Playing {
                            flags |= StreamFlags::INACTIVE
                        };

                        if stream
                            .connect(Direction::Output, None, flags, &mut [&newformatpod])
                            .is_err()
                        {
                            DeadBeef::log_detailed(
                                DDB_LOG_LAYER_DEFAULT,
                                "Pipewire: Unable to connect stream, terminating\n",
                            );
                            DeadBeef::sendmessage(DB_EV_STOP, 0, 0, 0);
                            return;
                        }

                        let rs = format!("1/{}", samplerate);
                        let props = properties! {
                            "node.rate" => rs,
                            "node.latency" => "1200/48000",
                        };
                        update_stream_props(&stream, &props);
                    }
                }
                PwThreadMessage::SetVol { newvol } => {
                    let values = [newvol];
                    stream
                        .set_control(libspa_sys::SPA_PROP_channelVolumes, &values)
                        .expect("Unable to set volume");
                }
                PwThreadMessage::SetTitle(title) => {
                    let props = properties! {
                        *pipewire::keys::MEDIA_NAME => title,
                    };
                    update_stream_props(&stream, &props);
                }
            };
        }
    });

    mainloop.run();
}
