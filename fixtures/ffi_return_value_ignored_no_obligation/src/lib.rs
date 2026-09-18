unsafe extern "C" {
    fn checkable() -> i32;
}

pub fn run() {
    // SAFETY: checkable takes no pointers and its return value is ignored.
    unsafe { checkable() };
}

#[cfg(test)]
mod tests {
    use super::run;

    #[test]
    fn mentions_run_wrapper() {
        let _wrapper = run as fn();
    }
}
