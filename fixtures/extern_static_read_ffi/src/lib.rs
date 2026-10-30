unsafe extern "C" {
    static ERRNO: i32;
}

pub fn current_errno() -> i32 {
    // SAFETY: ERRNO is set by the foreign runtime before this read.
    unsafe { ERRNO }
}

#[cfg(test)]
mod tests {
    use super::current_errno;

    #[test]
    fn mentions_current_errno() {
        let _ = current_errno as fn() -> i32;
    }
}
