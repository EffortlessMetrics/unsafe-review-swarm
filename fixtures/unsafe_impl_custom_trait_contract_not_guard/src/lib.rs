/// # Safety
///
/// Implementors must guarantee that `call` never aliases a live `&T` reference.
pub unsafe trait AliasFreeCall {
    fn call(&self);
}

pub struct Wrapper(u32);

// SAFETY: Wrapper does not alias any live reference when call is invoked.
unsafe impl AliasFreeCall for Wrapper {
    fn call(&self) {
        let _ = self.0;
    }
}

#[cfg(test)]
mod tests {
    use super::{AliasFreeCall, Wrapper};

    #[test]
    fn probe_custom_trait_impl() {
        let w = Wrapper(1);
        w.call();
    }
}
