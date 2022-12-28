use std::ffi::CString;
pub struct LossyCString;

impl LossyCString {
    #[allow(clippy::new_ret_no_self)]
    pub fn new<T: AsRef<str>>(t: T) -> CString {
        match CString::new(t.as_ref()) {
            Ok(cstr) => cstr,
            Err(_) => CString::new(t.as_ref().replace('\0', "")).expect("string has no nulls"),
        }
    }
}