use super::push_if_missing_discharge;
use crate::domain::{OperationFamily, ReviewCard};

pub(super) fn add_for_family(card: &ReviewCard, repairs: &mut Vec<String>) {
    match card.operation.family {
        OperationFamily::VecSetLen => add_vec_set_len_repairs(card, repairs),
        OperationFamily::MaybeUninitAssumeInit => {
            add_maybeuninit_assume_init_repairs(card, repairs)
        }
        OperationFamily::Transmute => add_transmute_repairs(card, repairs),
        OperationFamily::Zeroed => add_zeroed_repairs(card, repairs),
        OperationFamily::UnwrapUnchecked => add_unwrap_unchecked_repairs(card, repairs),
        OperationFamily::UnreachableUnchecked => add_unreachable_unchecked_repairs(card, repairs),
        OperationFamily::StrFromUtf8Unchecked => add_str_from_utf8_unchecked_repairs(card, repairs),
        OperationFamily::NonNullUnchecked => add_nonnull_unchecked_repairs(card, repairs),
        OperationFamily::GetUnchecked => add_get_unchecked_repairs(card, repairs),
        OperationFamily::BoxFromRaw => add_box_from_raw_repairs(card, repairs),
        _ => {}
    }
}

fn add_vec_set_len_repairs(card: &ReviewCard, repairs: &mut Vec<String>) {
    push_if_missing_discharge(
        card,
        repairs,
        "capacity",
        "add a same-vector capacity guard before `set_len` for the requested length",
    );
    push_if_missing_discharge(
        card,
        repairs,
        "initialized",
        "initialize the extended element range for this same vector and requested length before calling `set_len`",
    );
}

fn add_maybeuninit_assume_init_repairs(card: &ReviewCard, repairs: &mut Vec<String>) {
    push_if_missing_discharge(
        card,
        repairs,
        "initialized",
        "write or construct the same `MaybeUninit` slot before `assume_init`",
    );
    push_if_missing_discharge(
        card,
        repairs,
        "initialized",
        "keep the initialization branch open to the unsafe site and do not reassign the slot afterward",
    );
}

fn add_transmute_repairs(card: &ReviewCard, repairs: &mut Vec<String>) {
    push_if_missing_discharge(
        card,
        repairs,
        "layout",
        "prove the source and destination layouts are compatible before this transmute",
    );
    push_if_missing_discharge(
        card,
        repairs,
        "valid-value",
        "prove the source value is in the destination type's valid-value domain before this transmute",
    );
}

fn add_zeroed_repairs(card: &ReviewCard, repairs: &mut Vec<String>) {
    push_if_missing_discharge(
        card,
        repairs,
        "valid-zero",
        "prove the all-zero bit pattern is valid for this target type before `zeroed`",
    );
    push_if_missing_discharge(
        card,
        repairs,
        "valid-zero",
        "prefer an explicit constructor or `MaybeUninit` path when zero is not a valid value",
    );
}

fn add_unwrap_unchecked_repairs(card: &ReviewCard, repairs: &mut Vec<String>) {
    push_if_missing_discharge(
        card,
        repairs,
        "valid-value",
        "add a same-receiver `Some` or `Ok` guard on an open path before `unwrap_unchecked`",
    );
    push_if_missing_discharge(
        card,
        repairs,
        "valid-value",
        "preserve the same receiver value between the guard and `unwrap_unchecked`",
    );
}

fn add_unreachable_unchecked_repairs(card: &ReviewCard, repairs: &mut Vec<String>) {
    push_if_missing_discharge(
        card,
        repairs,
        "unreachable",
        "prove the same control-flow path is unreachable before `unreachable_unchecked`",
    );
    push_if_missing_discharge(
        card,
        repairs,
        "unreachable",
        "prefer a safe return, error, or panic path if reachability is uncertain",
    );
}

fn add_str_from_utf8_unchecked_repairs(card: &ReviewCard, repairs: &mut Vec<String>) {
    push_if_missing_discharge(
        card,
        repairs,
        "utf8",
        "validate the same byte buffer as UTF-8 on an open path before calling `from_utf8_unchecked`",
    );
    push_if_missing_discharge(
        card,
        repairs,
        "utf8",
        "preserve the same byte buffer between validation and the unchecked conversion",
    );
}

fn add_nonnull_unchecked_repairs(card: &ReviewCard, repairs: &mut Vec<String>) {
    push_if_missing_discharge(
        card,
        repairs,
        "non-null",
        "add a same-pointer non-null guard before `NonNull::new_unchecked`",
    );
    push_if_missing_discharge(
        card,
        repairs,
        "non-null",
        "preserve the same pointer value between the guard and `NonNull::new_unchecked`",
    );
}

fn add_get_unchecked_repairs(card: &ReviewCard, repairs: &mut Vec<String>) {
    push_if_missing_discharge(
        card,
        repairs,
        "bounds",
        "add a same-slice length/range guard before `get_unchecked` for the same index",
    );
    push_if_missing_discharge(
        card,
        repairs,
        "bounds",
        "preserve the same index value between the guard and unchecked access",
    );
}

fn add_box_from_raw_repairs(card: &ReviewCard, repairs: &mut Vec<String>) {
    push_if_missing_discharge(
        card,
        repairs,
        "ownership",
        "prove the same raw pointer came from `Box::into_raw` with a compatible allocator before `Box::from_raw`",
    );
    push_if_missing_discharge(
        card,
        repairs,
        "ownership",
        "show unique ownership of that pointer so it will not be double-freed or reused after reconstruction",
    );
}
