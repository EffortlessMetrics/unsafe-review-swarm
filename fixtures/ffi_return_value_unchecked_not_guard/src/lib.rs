unsafe extern "C" {
    fn checkable() -> i32;
}

pub fn run() -> i32 {
    // SAFETY: checkable takes no pointers; its return value is used unchecked.
    let result = unsafe { checkable() };
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
