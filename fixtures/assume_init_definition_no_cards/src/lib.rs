/// A safe wrapper that names itself `assume_init` but contains no unsafe code.
/// The function definition header must not be carded as a MaybeUninit::assume_init call.
pub fn assume_init(x: u8) -> u8 {
    x
}
