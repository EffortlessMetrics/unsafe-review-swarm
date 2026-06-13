unsafe extern "C" {
    fn close(fd: i32) -> i32;
}

pub struct MyFile {
    fd: i32,
}

impl MyFile {
    /// # Safety
    ///
    /// The caller must ensure the file descriptor was opened by this instance.
    pub unsafe fn close(&mut self) -> i32 {
        self.fd
    }
}

pub fn shut(f: &mut MyFile) -> i32 {
    // SAFETY: the caller guarantees the MyFile was properly opened.
    unsafe { f.close() }
}

#[cfg(test)]
mod tests {
    use super::shut;

    #[test]
    fn mentions_shut() {
        let _ = shut as fn(&mut _) -> i32;
    }
}
