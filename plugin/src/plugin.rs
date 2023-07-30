use crate::*;

use std::{thread, cell::Cell};
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
        format: ddb_waveformat_t,
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
        if self.requested_fmt.is_none() {
            self.requested_fmt = Some(get_default_waveformat());
        }

        self.plugin.fmt = self.requested_fmt.unwrap();

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
        self.plugin.fmt = if fmt.channels == 0 {
            get_default_waveformat()
        } else {
            fmt
        };
        self.requested_fmt = Some(self.plugin.fmt);
        print_db_format(fmt);
        self.msgtothread(PwThreadMessage::SetFmt { format: fmt });
    }

    #[allow(unused)]
    fn message(&mut self, msgid: u32, ctx: usize, p1: u32, p2: u32) {
        match msgid {
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

fn set_channel_map(channels: u32, audioinfo: &mut libspa_sys::spa_audio_info_raw) {
    if channels == 1 {
        audioinfo.position[0] = libspa_sys::SPA_AUDIO_CHANNEL_MONO;
    }
    if channels >= 2 {
        audioinfo.position[0] = libspa_sys::SPA_AUDIO_CHANNEL_FL;
        audioinfo.position[1] = libspa_sys::SPA_AUDIO_CHANNEL_FR;
    }
    if channels >= 3 {
        audioinfo.position[2] = libspa_sys::SPA_AUDIO_CHANNEL_FC;
    }
    if channels >= 4 {
        audioinfo.position[3] = libspa_sys::SPA_AUDIO_CHANNEL_LFE;
    }
    if channels >= 6 {
        audioinfo.position[4] = libspa_sys::SPA_AUDIO_CHANNEL_RL;
        audioinfo.position[5] = libspa_sys::SPA_AUDIO_CHANNEL_RR;
    }
    if channels >= 8 {
        audioinfo.position[6] = libspa_sys::SPA_AUDIO_CHANNEL_FLC;
        audioinfo.position[7] = libspa_sys::SPA_AUDIO_CHANNEL_FRC;
    }
    if channels >= 9 {
        audioinfo.position[8] = libspa_sys::SPA_AUDIO_CHANNEL_RC;
    }
    if channels >= 11 {
        audioinfo.position[9] = libspa_sys::SPA_AUDIO_CHANNEL_SL;
        audioinfo.position[10] = libspa_sys::SPA_AUDIO_CHANNEL_SR;
    }
    if channels >= 12 {
        audioinfo.position[11] = libspa_sys::SPA_AUDIO_CHANNEL_TC;
    }
    if channels >= 15 {
        audioinfo.position[12] = libspa_sys::SPA_AUDIO_CHANNEL_TFL;
        audioinfo.position[13] = libspa_sys::SPA_AUDIO_CHANNEL_TFC;
        audioinfo.position[14] = libspa_sys::SPA_AUDIO_CHANNEL_TFR;

    }
    if channels >= 18 {
        audioinfo.position[15] = libspa_sys::SPA_AUDIO_CHANNEL_TRL;
        audioinfo.position[16] = libspa_sys::SPA_AUDIO_CHANNEL_TRC;
        audioinfo.position[17] = libspa_sys::SPA_AUDIO_CHANNEL_TRR;
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
            position: [libspa_sys::SPA_AUDIO_CHANNEL_UNKNOWN; 64],
        };
        set_channel_map(channels, &mut audioinfo);

        libspa_sys::spa_format_audio_raw_build(&mut b as *mut libspa_sys::spa_pod_builder,
            libspa_sys::SPA_PARAM_EnumFormat,
            &mut audioinfo as *mut libspa_sys::spa_audio_info_raw)
    }
}

fn pw_thread_main(init_fmt: ddb_waveformat_t, pw_receiver: pipewire::channel::Receiver<PwThreadMessage>) {
    let mainloop = MainLoop::new().expect("Failed to create mainloop");
    let context = Context::new(&mainloop).expect("Context");
    let core = context.connect(None).expect("Core");

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
        props.insert(*pipewire::keys::TARGET_OBJECT, &device);
    }

    let ourdisconnect = Rc::new(Cell::new(false));
    let buffersize = Rc::new(Cell::new((init_fmt.bps/8 * init_fmt.channels * 25 * init_fmt.samplerate/1000) as usize));

    let mut stream: stream::Stream<()> = match pipewire::stream::Stream::new(
        &core,
        "deadbeef",
        props,
    ) {
        Ok(a) => a,
        Err(e) => {    
            DeadBeef::log_detailed(DDB_LOG_LAYER_DEFAULT, format!("Pipewire: Unable to create stream, {}\n", e.to_string()).as_str());
            DeadBeef::sendmessage(DB_EV_STOP, 0, 0, 0);
            return;
        }
    };
    
    let _listener = stream.add_local_listener()
    .state_changed({
        let ourdisconnect = ourdisconnect.clone();
        move |old, new| {
            println!("State changed: {old:?} -> {new:?}");
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
    .process({
        let buffersize = buffersize.clone();
        move |stream, _user_data| {
            match stream.dequeue_buffer() {
                None => println!("No buffer received"),
                Some(mut buffer) => {
                    let datas = buffer.datas_mut();

                    let maxsize = datas[0].as_raw().maxsize as usize;
                    if let Some(d) = datas[0].data() {

                        let len = buffersize.get().min(maxsize);

                        let bytesread = if DeadBeef::streamer_ok_to_read(-1) > 0 {
                            DeadBeef::streamer_read(d.as_mut_ptr() as *mut c_void, len)
                        } else {
                            0
                        };

                        *datas[0].chunk_mut().size_mut() = bytesread as u32;
                        *datas[0].chunk_mut().offset_mut() = 0;
                        *datas[0].chunk_mut().stride_mut() = 1;

                        if bytesread == 0 {
                            buffer.queue();
                            stream.flush(false).expect("flush");
                        }
                    }
                }
            };
        }
    })
    .control_info(
        move |id, control_ptr: *const pipewire::sys::pw_stream_control| {
        if id == libspa_sys::SPA_PROP_channelVolumes {
            unsafe {
                let control = *control_ptr;
                if control.n_values > 0 {
                    let values = std::slice::from_raw_parts(control.values, control.n_values as usize);
                    for v in values.iter() {
                        if *v != DeadBeef::volume_get_amp() {
                            DeadBeef::volume_set_amp(*v);
                            break;
                        }
                    }
                }
            }
        }
    }).register();

    let mut buffer = [0;1024];
    let fmtpod = {
        let fmt = init_fmt;
        let format = db_format_to_pipewire(fmt);
        let channels = fmt.channels as u32;
        let rate = fmt.samplerate as u32;

        create_audio_format_pod(format, channels, rate, &mut buffer)
    };

    if let Err(e) = stream.connect(
        pipewire::spa::Direction::Output,
        None,
        stream::StreamFlags::AUTOCONNECT
            | stream::StreamFlags::MAP_BUFFERS
            | stream::StreamFlags::RT_PROCESS,
        &mut [fmtpod],
    ) {
        DeadBeef::log_detailed(DDB_LOG_LAYER_DEFAULT, format!("Pipewire: Unable to connect stream, {}\n", e.to_string()).as_str());
        DeadBeef::sendmessage(DB_EV_STOP, 0, 0, 0);
        return;
    }

    // When we receive a `Terminate` message, quit the main loop.
    let _receiver = pw_receiver.attach(&mainloop, {
        let mainloop = mainloop.clone();
        move |msg| {
            match msg {
                PwThreadMessage::Terminate => {
                    ourdisconnect.set(true);
                    mainloop.quit();
                },
                PwThreadMessage::Pause => stream.set_active(false).unwrap(),
                PwThreadMessage::Unpause => stream.set_active(true).unwrap(),
                PwThreadMessage::SetFmt { format } => {
                    ourdisconnect.set(true);
                    stream.set_active(false).unwrap();
                    if stream.disconnect().is_ok() {

                        print!("Set format called with: ");
                        let pwfmt = db_format_to_pipewire(format);
                        let channels = format.channels as u32;
                        let samplerate = format.samplerate as u32;
                        print_pipewire_format(pwfmt, channels, samplerate);
    
                        let mut buffer = [0;1024];
                        let newformatpod: *mut libspa_sys::spa_pod = create_audio_format_pod(pwfmt, channels, samplerate, &mut buffer);
                        buffersize.set((format.bps/8 * format.channels * 25 * format.samplerate/1000) as usize);

                        if stream.connect(
                            pipewire::spa::Direction::Output,
                            None,
                            stream::StreamFlags::AUTOCONNECT
                                | stream::StreamFlags::MAP_BUFFERS
                                | stream::StreamFlags::RT_PROCESS,
                            &mut [newformatpod],
                        ).is_err() {
                            DeadBeef::log_detailed(DDB_LOG_LAYER_DEFAULT, "Pipewire: Unable to connect stream, terminating\n");
                            DeadBeef::sendmessage(DB_EV_STOP, 0, 0, 0);
                            return;
                        }

                        let s = format!("1/{}", samplerate);
                        let props = properties!{
                            "node.rate" => s
                        };
                        unsafe {
                            pipewire::sys::pw_stream_update_properties(stream.as_ptr(), props.get_dict_ptr());
                        }
                    }
                },
                PwThreadMessage::SetVol { newvol } => {
                    let values = [newvol];
                    stream.set_control(libspa_sys::SPA_PROP_channelVolumes, &values).expect("Unable to set volume");
                }
            };
        }
    });

    mainloop.run();

}
