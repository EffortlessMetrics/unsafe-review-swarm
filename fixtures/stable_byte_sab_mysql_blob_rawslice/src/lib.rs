pub struct JSValue {
    ptr: *const u8,
    len: usize,
    shared: bool,
}

impl JSValue {
    pub fn is_shared_array_buffer(&self) -> bool {
        self.shared
    }

    pub fn borrow_bytes_for_off_thread(&self) -> (*const u8, usize) {
        (self.ptr, self.len)
    }
}

pub struct RawSlice {
    ptr: *const u8,
    len: usize,
}

pub enum Data {
    Temporary(RawSlice),
    Empty,
}

pub fn mysql_blob_sab_bind(value: &JSValue) -> Data {
    if value.is_shared_array_buffer() {
        let (ptr, len) = value.borrow_bytes_for_off_thread();
        let bytes = unsafe { core::slice::from_raw_parts(ptr, len) };
        let raw = RawSlice {
            ptr: bytes.as_ptr(),
            len: bytes.len(),
        };
        return Data::Temporary(raw);
    }

    Data::Empty
}
