use pipewire::{prelude::*, Properties, spa::param::audio::AudioFormat};

use crate::ddb_waveformat_t;

pub fn db_format_to_pipewire(input: ddb_waveformat_t) -> AudioFormat {
    match input.bps {
        8 => AudioFormat::S8,
        16 => AudioFormat::S16LE,
        24 => AudioFormat::S24LE,
        32 => match input.is_float == 1 {
            true => AudioFormat::F32LE,
            false => AudioFormat::S32LE,
        },
        _ => AudioFormat::Unknown,
    }
}

#[cfg(debug_assertions)]
pub fn print_db_format(input: ddb_waveformat_t) {
    println!(
        "db format: {} bps{}, {} channels, {} kHz",
        input.bps,
        if input.is_float == 1 { " float" } else { "" },
        input.channels,
        input.samplerate
    );
}
#[cfg(not(debug_assertions))]
pub fn print_db_format(_input: ddb_waveformat_t) {}

#[cfg(debug_assertions)]
pub fn print_pipewire_format(format: AudioFormat, channels: u32, rate: u32) {
    println!(
        "pw format: {}, {} channels, {} kHz",
        match format {
            AudioFormat::S8 => "8 bps",
            AudioFormat::S16LE => "16 bps",
            AudioFormat::S24LE => "24 bps",
            AudioFormat::F32LE => "32 bps float",
            AudioFormat::S32LE => "32 bps",
            _ => "unknown bps",
        },
        channels,
        rate
    );
}

#[cfg(not(debug_assertions))]
pub fn print_pipewire_format(_format: AudioFormat, _channels: u32, _rate: u32) {}


// Standalone function for this instead of "impl Default" because ddb_waveformat_t is generated by bindgen
pub fn get_default_waveformat() -> ddb_waveformat_t {
    ddb_waveformat_t {
        samplerate: 44100,
        bps: 16,
        channels: 2,
        channelmask: 3,
        flags: 0,
        is_float: 0,
    }
}

pub fn update_stream_props(stream: &pipewire::stream::Stream, props: &Properties) {
    unsafe {
        pipewire::sys::pw_stream_update_properties(stream.as_raw_ptr(), props.get_dict_ptr());
    }
}

// macro_rules! lit_cstr {
//     ($s:expr) => {
//         (concat!($s, "\0").as_bytes().as_ptr() as *const c_char)
//     };
// }

macro_rules! debug {
    ($s:expr) => {
        {
        #[cfg(debug_assertions)]
        eprintln!($s)
        }
    };
}
