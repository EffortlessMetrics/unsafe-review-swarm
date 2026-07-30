// A SIMD dispatch module: three architecture-specific accumulation
// functions that all share the same missing-safety-contract shape, plus one
// deliberately different, documented AVX-512 variant. This is the fixture
// target for the target-feature-summary grouping projection (issue #1894)
// -- it proves the summary groups the three undocumented arch variants into
// one row (3 sites) while the fourth, documented `sum_avx512` site keeps its
// own distinct class and is never collapsed into that group. Every card
// still keeps its own ID, class, and location in `cards.json`.

#[target_feature(enable = "avx2")]
pub fn sum_avx2(data: &[f32]) -> f32 {
    data.iter().sum()
}

#[target_feature(enable = "sse2")]
pub fn sum_sse2(data: &[f32]) -> f32 {
    data.iter().sum()
}

#[target_feature(enable = "neon")]
pub fn sum_neon(data: &[f32]) -> f32 {
    data.iter().sum()
}

/// Runs the AVX-512 accumulation path.
///
/// # Safety
///
/// Callers must only execute this function when AVX-512F is available.
#[target_feature(enable = "avx512f")]
pub fn sum_avx512(data: &[f32]) -> f32 {
    data.iter().sum()
}
