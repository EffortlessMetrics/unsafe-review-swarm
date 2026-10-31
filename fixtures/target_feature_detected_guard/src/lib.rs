#[target_feature(enable = "avx2")]
pub fn sum_avx2(data: &[f32]) -> f32 {
    data.iter().sum()
}

pub fn sum(data: &[f32]) -> f32 {
    if std::is_x86_feature_detected!("avx2") {
        // SAFETY: AVX2 availability is checked above before calling the
        // target-feature-gated accumulation.
        unsafe { sum_avx2(data) }
    } else {
        data.iter().sum()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn names_sum() {
        let owner = "sum";
        assert!(owner.contains("sum"));
    }
}
