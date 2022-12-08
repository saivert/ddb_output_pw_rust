use std::ffi::CString;

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


macro_rules! lit_cstr {
    ($s:literal) => {
        (concat!($s, "\0").as_bytes().as_ptr() as *const c_char)
    };
}


