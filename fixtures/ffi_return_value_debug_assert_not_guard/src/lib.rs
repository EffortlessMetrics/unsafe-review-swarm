unsafe extern "C" {
    fn checkable() -> i32;
}

pub fn run() -> i32 {
    // SAFETY: checkable takes no pointers; debug-only validation is not a guard.
    let result = unsafe { checkable() };
    debug_assert!(result == 0);
    result
}

#[cfg(test)]
mod tests {
    use super::run;

    #[test]
    fn mentions_run_wrapper() {
        let _wrapper = run as fn() -> i32;
    }
}
