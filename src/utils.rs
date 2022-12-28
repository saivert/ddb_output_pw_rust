use crate::ddb_waveformat_t;

pub fn db_format_to_pipewire(input: ddb_waveformat_t) -> u32 {
    match input.bps {
        8 => libspa_sys::SPA_AUDIO_FORMAT_S8,
        16 => libspa_sys::SPA_AUDIO_FORMAT_S16_LE,
        24 => libspa_sys::SPA_AUDIO_FORMAT_S24_LE,
        32 => match input.is_float == 1 {
            true => libspa_sys::SPA_AUDIO_FORMAT_F32_LE,
            false => libspa_sys::SPA_AUDIO_FORMAT_S32_LE,
        },
        _ => libspa_sys::SPA_AUDIO_FORMAT_UNKNOWN,
    }
}

pub fn print_db_format(input: ddb_waveformat_t) {
    println!("db format: {} bps{}, {} channels, {} kHz",
        input.bps,
        if input.is_float == 1 {" float"} else {""},
        input.channels,
        input.samplerate);
}

pub fn print_pipewire_format(format: u32, channels: u32, rate: u32) {
    println!("pw format: {}, {} channels, {} kHz", match format {
        libspa_sys::SPA_AUDIO_FORMAT_S8 => "8 bps",
        libspa_sys::SPA_AUDIO_FORMAT_S16_LE => "16 bps",
        libspa_sys::SPA_AUDIO_FORMAT_S24_LE => "24 bps",
        libspa_sys::SPA_AUDIO_FORMAT_F32_LE => "32 bps float",
        libspa_sys::SPA_AUDIO_FORMAT_S32_LE => "32 bps",
        _ => "unknown bps"
    }, channels, rate);
}

macro_rules! lit_cstr {
    ($s:expr) => {
        (concat!($s, "\0").as_bytes().as_ptr() as *const c_char)
    };
}
