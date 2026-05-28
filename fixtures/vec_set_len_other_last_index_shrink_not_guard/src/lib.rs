pub fn shrink_from_other(values: &mut Vec<u8>, other: &Vec<u8>) {
    if other.len() == 0 {
        return;
    }
    let last_index = other.len() - 1;
    // SAFETY: this fixture mentions shrink-style evidence for `other`, not
    // the vector whose length is changed.
    unsafe {
        values.set_len(last_index);
    }
}

#[cfg(test)]
mod tests {
    use super::shrink_from_other;

    #[test]
    fn mentions_shrink_from_other() {
        let mut values = Vec::with_capacity(1);
        let other = vec![1];
        shrink_from_other(&mut values, &other);
    }
}

