static mut COUNTER: u32 = 0;

pub fn bump() -> u32 {
    // SAFETY: callers serialize access to COUNTER.
    unsafe { COUNTER += 1; COUNTER }
}

#[cfg(test)]
mod tests {
    use super::bump;

    #[test]
    fn mentions_bump() {
        let _ = bump as fn() -> u32;
    }
}
