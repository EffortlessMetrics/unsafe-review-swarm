pub fn copy_checked(src: &[u8], dst: &mut [u8], count: usize) {
    if src.len() >= count {
        if dst.len() >= count {
            let mut other = [0_u8; 1];
            let dst = &mut other[..];
            // SAFETY: fixture checks that shadowed open-branch destination guards are not range evidence.
            unsafe { core::ptr::copy_nonoverlapping(src.as_ptr(), dst.as_mut_ptr(), count) }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::copy_checked;

    #[test]
    fn mentions_copy_checked() {
        let _ = stringify!(copy_checked);
    }
}

