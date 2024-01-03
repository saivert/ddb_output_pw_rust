#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]
#![allow(clippy::all)]

use lossycstring::LossyCString;
use once_cell::sync::OnceCell;
use core::panic;
use std::ffi::{c_char, c_void, CString};

use std::ptr;
use thiserror::Error;

pub static mut DEADBEEF: Option<DeadBeef> = None;
static mut DEADBEEF_THREAD_ID: Option<std::thread::ThreadId> = None;

#[allow(deref_nullptr)]
mod api {
include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
}
pub use api::*;

mod plugin_struct;
use plugin_struct::*;

/// Main DeadBeef struct that encapsulates common DeadBeef API functions.
pub struct DeadBeef {
    pub(crate) ptr: *const DB_functions_t,
    pub(crate) plugin_ptr: *mut DB_plugin_t,
}

pub trait DBOutputPluginCreate {
    fn new(plugin: DB_output_t) -> Self ;
}

pub trait DBMiscPluginCreate {
    fn new(plugin: DB_misc_t) -> Self ;
}

pub trait DBPlugin {
    fn get_plugin_ptr(&self) -> *mut DB_plugin_t;

    fn plugin_start(&mut self);
    fn plugin_stop(&mut self);
    fn message(&mut self, msgid: u32, ctx: usize, p1: u32, p2: u32);

    fn as_output(&mut self) -> Option<&mut dyn DBOutput> {None}
}

pub struct EnumSoundcard {
    callback: unsafe extern "C" fn(name: *const c_char, desc: *const c_char, _userdata: *mut c_void),
    userdata: *mut c_void,
}

impl EnumSoundcard  {
    pub fn add_card(&self, name: &str, description: &str) {
        let name = LossyCString::new(name);
        let desc = LossyCString::new(description);

        unsafe {
            (self.callback)(name.as_ptr(), desc.as_ptr(), self.userdata);
        }
    }
}

pub trait DBOutput: DBPlugin {
    fn init(&mut self) -> i32 {0}
    fn free(&mut self);
    fn play(&mut self);
    fn stop(&mut self);
    fn pause(&mut self);
    fn unpause(&mut self);
    fn getstate(&self) -> ddb_playback_state_e;
    fn setformat(&mut self, fmt: ddb_waveformat_t);
    fn enum_soundcards(&self, callback: EnumSoundcard);
}

#[derive(Error, Debug)]
pub enum DB_TF_Error {
    #[error("Compile error")]
    CompileError,
    #[error("Evaluation error")]
    EvalError,
    #[error(transparent)]
    DBError(#[from] DB_Error),
}

#[derive(Error, Debug)]
pub enum DB_Error {
    #[error("Creation failed")]
    CreationFailed,
    #[error("No memory")]
    NoMemory,
}

impl DeadBeef {
    pub fn create_output_plugin<T>(api: *const DB_functions_t, id: &str, name: &str, description: &str, copyright: &str, website: &str) -> *mut DB_plugin_t
    where T: DBOutputPluginCreate + DBPlugin + 'static {
        assert!(!api.is_null());

        unsafe {
            DEADBEEF = Some(DeadBeef { ptr: api, plugin_ptr: std::ptr::null_mut() as *mut DB_plugin_t });
            DEADBEEF_THREAD_ID = Some(std::thread::current().id());
        }

        let wrapper = PluginStructWrapper {
            id: CString::new(id).unwrap(),
            name: CString::new(name).unwrap(),
            description: CString::new(description).unwrap(),
            copyright: CString::new(copyright).unwrap(),
            website: CString::new(website).unwrap(),
            plugin: OnceCell::default(),
        };

        let db_plugin_struct = DB_output_t {
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
                name: wrapper.name.as_ptr(),
                descr: wrapper.description.as_ptr(),
                copyright: wrapper.copyright.as_ptr(),
                website: wrapper.website.as_ptr(),
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
        
        if let Ok(p) = unsafe { &mut PLUGIN.lock() } {
            let plugin = Box::new(T::new(db_plugin_struct));
            let ptr = plugin.get_plugin_ptr();
            wrapper.plugin.set(plugin).expect("Not set");
            p.set(wrapper).expect("Plugin wrapper not set");
            return ptr;
        }

        std::ptr::null_mut() as *mut DB_plugin_t
    }

    pub fn create_misc_plugin<T>(api: *const DB_functions_t, id: &str, name: &str, description: &str, copyright: &str, website: &str) -> *mut DB_plugin_t
    where T: DBMiscPluginCreate + DBPlugin + 'static {
        assert!(!api.is_null());

        unsafe {
            DEADBEEF = Some(DeadBeef { ptr: api, plugin_ptr: std::ptr::null_mut() as *mut DB_plugin_t });
            DEADBEEF_THREAD_ID = Some(std::thread::current().id());
        }

        let wrapper = PluginStructWrapper {
            id: CString::new(id).unwrap(),
            name: CString::new(name).unwrap(),
            description: CString::new(description).unwrap(),
            copyright: CString::new(copyright).unwrap(),
            website: CString::new(website).unwrap(),
            plugin: OnceCell::default(),
        };

        let db_plugin_struct = DB_misc_t {
            plugin: DB_plugin_t {
                api_vmajor: 1,
                api_vminor: 0,
                version_major: 0,
                version_minor: 1,
                flags: DDB_PLUGIN_FLAG_LOGGING,
                type_: DB_PLUGIN_MISC as i32,
                id: wrapper.id.as_ptr(),
                name: wrapper.name.as_ptr(),
                descr: wrapper.description.as_ptr(),
                copyright: wrapper.copyright.as_ptr(),
                website: wrapper.website.as_ptr(),
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
        
        if let Ok(p) = unsafe { &mut PLUGIN.lock() } {
            let plugin = Box::new(T::new(db_plugin_struct));
            let ptr = plugin.get_plugin_ptr();
            wrapper.plugin.set(plugin).expect("Not set");
            p.set(wrapper).expect("Plugin wrapper not set");
            return ptr;
        }

        std::ptr::null_mut() as *mut DB_plugin_t
    }

    pub fn set_plugin_ptr(ptr: *mut DB_plugin_t) {
        let deadbeef = unsafe { DeadBeef::deadbeef() };
        deadbeef.plugin_ptr = ptr;
    }

    pub unsafe fn deadbeef() -> &'static mut DeadBeef {
        match DEADBEEF {
            Some(ref mut w) => w,
            None => panic!("Plugin wasn't initialized correctly"),
        }
    }

    pub(crate) fn check_thread() {
        let deadbeef_thread_id = unsafe {
            DEADBEEF_THREAD_ID.as_ref().expect(
                "DeadBeef main thread ID wasn't found, plugin \
                 wasn't correctly initialized",
            )
        };

        if std::thread::current().id() != *deadbeef_thread_id {
            panic!(
                "Deadbeef methods can be only called from the main Deadbeef \
                 thread."
            )
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
            log_detailed(deadbeef.plugin_ptr as *mut DB_plugin_t, layers, msg.as_ptr());
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

    pub fn conf_get_str(item: impl Into<String>, default: impl Into<String>) -> String {
        let deadbeef = unsafe { DeadBeef::deadbeef() };

        let item = LossyCString::new(item.into());
        let default = LossyCString::new(default.into());
        let conf_get_str = deadbeef.get().conf_get_str.unwrap();
        let mut buf: Vec<u8> = vec![0; 4096];

        unsafe { conf_get_str(item.as_ptr(), default.as_ptr(), buf.as_mut_ptr() as *mut i8, 4096); }

        let cstr = std::ffi::CStr::from_bytes_until_nul(&buf);
        return cstr.expect("null terminated string").to_string_lossy().to_string();
    }

    pub fn volume_set_amp(vol: f32) {
        let deadbeef = unsafe { DeadBeef::deadbeef() };
        let volume_set_amp = deadbeef.get().volume_set_amp.unwrap();

        unsafe { volume_set_amp(vol); }
    }

    pub fn volume_get_amp() -> f32 {
        let deadbeef = unsafe { DeadBeef::deadbeef() };
        let volume_get_amp = deadbeef.get().volume_get_amp.unwrap();

        unsafe { volume_get_amp() }
    }

    pub fn current_track() -> Result<PlItem, DB_Error>  {
        let deadbeef = unsafe { DeadBeef::deadbeef() };
        let streamer_get_playing_track_safe = deadbeef.get().streamer_get_playing_track_safe.unwrap();

        let it = unsafe { streamer_get_playing_track_safe() };

        PlItem::from_raw(it)
    }

    pub fn titleformat(format: impl Into<String>) -> Result<String, DB_TF_Error> {
        let track = Self::current_track()?;
        Self::titleformat_for_item(format, &track)
    }
    
    pub fn titleformat_for_item(format: impl Into<String>, item: &PlItem) -> Result<String, DB_TF_Error> {
        let deadbeef = unsafe { DeadBeef::deadbeef() };

        let format = LossyCString::new(format.into());

        let tf_compile = deadbeef.get().tf_compile.unwrap();
        let tf_eval = deadbeef.get().tf_eval.unwrap();
        let tf_free = deadbeef.get().tf_free.unwrap();

        let mut buf: Vec<u8> = vec![0; 4096];

        unsafe {
            let tf = tf_compile(format.as_ptr());
            if tf <= std::ptr::null_mut() {
                return Err(DB_TF_Error::CompileError);
            }
            let mut ctx = ddb_tf_context_t {
                _size: std::mem::size_of::<ddb_tf_context_t>() as i32,
                flags: DDB_TF_CONTEXT_NO_DYNAMIC,
                it: item.as_ptr(),
                ..Default::default()
            };
            if tf_eval(&mut ctx as *mut _, tf, buf.as_mut_ptr() as *mut i8, 4096) <= 0 {
                return Err(DB_TF_Error::EvalError);
            }
            tf_free(tf);
        }
        let cstr = std::ffi::CStr::from_bytes_until_nul(&buf);
        Ok(cstr.expect("null terminated string").to_string_lossy().to_string())
    }


}


pub struct PlItem {
    ptr: ptr::NonNull<DB_playItem_s>,
}

impl PlItem {
    pub fn from_raw(fromptr: *mut DB_playItem_s) -> Result<Self, DB_Error> {
        let ptr: ptr::NonNull<DB_playItem_s> = ptr::NonNull::new(fromptr).ok_or(DB_Error::CreationFailed)?;
        Ok(Self {
            ptr,
        })
    }

    pub fn pl_item_unref(item: *mut DB_playItem_s) {
        let deadbeef = unsafe { DeadBeef::deadbeef() };
        let pl_item_unref = deadbeef.get().pl_item_unref.unwrap();

        unsafe { pl_item_unref(item); }
    }

    fn as_ptr(&self) -> *mut DB_playItem_s {
        self.ptr.as_ptr()
    }
}

impl std::ops::Drop for PlItem {
    fn drop(&mut self) {
        PlItem::pl_item_unref(self.ptr.as_ptr());
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct PlaybackState(ddb_playback_state_e);

impl PlaybackState {
    pub const Playing: Self = Self(DDB_PLAYBACK_STATE_PLAYING);
    pub const Stopped: Self = Self(DDB_PLAYBACK_STATE_STOPPED);
    pub const Paused: Self = Self(DDB_PLAYBACK_STATE_PAUSED);

    pub fn from_raw(raw: ddb_playback_state_e) -> Self {
        Self(raw)
    }

    pub fn as_raw(&self) -> ddb_playback_state_e {
        self.0
    }
}

impl std::fmt::Debug for PlaybackState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = format!(
            "PlaybackState::{}",
            match *self {
                Self::Playing => "Playing",
                Self::Paused => "Paused",
                Self::Stopped => "Stopped",
                _ => "Unknown",
            }
        );
        f.write_str(&name)
    }
}