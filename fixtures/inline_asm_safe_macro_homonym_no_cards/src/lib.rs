//! D5 negative control: a user-defined `asm!` macro homonym called inside an
//! `unsafe` block. The bare `line.contains("asm!")` detector would match this,
//! but the call is to a safe user macro, not `core::arch::asm!`.
//!
//! The fixture stresses whether the detector distinguishes the stdlib `asm!`
//! from a same-named user macro. If the detector false-positives here, the
//! inline_asm contract's D5 (call-name anchoring) obligation is unmet.

/// A safe user-defined macro that happens to share the `asm!` name. It performs
/// no inline assembly — it is a logging stub.
macro_rules! asm {
    () => {
        println!("asm! stub: no actual inline assembly");
    };
}

/// Caller inside an `unsafe` block invoking the safe user `asm!` macro.
/// This must NOT produce an `inline_asm` card.
pub fn call_safe_asm_macro() {
    // SAFETY: the `asm!` invocation here is the safe user macro above, not
    // `core::arch::asm!`. No inline assembly is executed.
    unsafe {
        asm!();
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn mentions_caller() {
        let _ = stringify!(call_safe_asm_macro);
    }
}
