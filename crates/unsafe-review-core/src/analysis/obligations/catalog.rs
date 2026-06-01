mod boundary;
mod memory;
mod pointer;
mod value;

use crate::domain::{OperationFamily, SafetyObligation};

type ObligationSpec = (&'static str, &'static str);

pub(crate) fn obligations_for(family: &OperationFamily) -> Vec<SafetyObligation> {
    pointer::obligations(family)
        .or_else(|| memory::obligations(family))
        .or_else(|| value::obligations(family))
        .or_else(|| boundary::obligations(family))
        .unwrap_or_else(unknown_obligations)
}

fn from_specs(specs: &[ObligationSpec]) -> Vec<SafetyObligation> {
    specs
        .iter()
        .map(|(key, description)| SafetyObligation::new(*key, *description))
        .collect()
}

fn unknown_obligations() -> Vec<SafetyObligation> {
    from_specs(&[(
        "unknown",
        "unsafe contract could not be inferred from this syntax shape",
    )])
}
