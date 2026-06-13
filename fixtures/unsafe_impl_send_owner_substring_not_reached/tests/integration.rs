// Negative-control: only the longer type SharedCellar is mentioned here.
// Every occurrence of the owner-name substring appears inside SharedCellar,
// never as a standalone identifier, so reach must not be credited.
use unsafe_impl_send_owner_substring_not_reached::SharedCellar;
use core::cell::UnsafeCell;

#[test]
fn constructs_shared_cellar() {
    let _cellar = SharedCellar { value: UnsafeCell::new(0) };
}
