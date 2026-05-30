pub struct One {
    needle: u8,
}

impl One {
    pub fn is_available() -> bool {
        true
    }

    pub unsafe fn new_unchecked(needle: u8) -> One {
        One { needle }
    }

    pub fn new(needle: u8) -> Option<One> {
        if !One::is_available() {
            // return None before constructing this searcher when unavailable.
            observe_unavailable_constructor();
        }
        // SAFETY: this fixture intentionally leaves the unavailable branch without an executable return guard.
        Some(unsafe { One::new_unchecked(needle) })
    }
}

fn observe_unavailable_constructor() {}

#[cfg(test)]
mod tests {
    use super::One;

    #[test]
    fn constructs_when_available() {
        let _ = One::new(7);
    }
}
