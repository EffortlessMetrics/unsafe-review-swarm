unsafe extern "C" {
    fn checkable() -> i32;
}

pub fn run() -> Result<(), &'static str> {
    // SAFETY: checkable takes no pointers; only its return value needs validation.
    let result = unsafe { checkable() };
    if result == 0 {
        Ok(())
    } else {
        Err("checkable failed")
    }
}

#[cfg(test)]
mod tests {
    use super::run;

    #[test]
    fn mentions_run_wrapper() {
        let _wrapper = run as fn() -> Result<(), &'static str>;
    }
}
