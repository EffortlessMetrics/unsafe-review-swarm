pub fn store_nomem(p: *mut u32) {
    // SAFETY: fixture declares nomem while the template stores through memory.
    unsafe { core::arch::asm!("mov [{}], {}", in(reg) p, in(reg) 7u32, options(nomem, nostack)) }
}

pub fn store_readonly(p: *mut u32) {
    // SAFETY: fixture declares readonly while the destination operand writes memory.
    unsafe { core::arch::asm!("mov [{}], {}", in(reg) p, in(reg) 7u32, options(readonly, nostack)) }
}

#[cfg(test)]
mod tests {
    #[test]
    fn mentions_stores() {
        let _ = stringify!(store_nomem);
        let _ = stringify!(store_readonly);
    }
}
