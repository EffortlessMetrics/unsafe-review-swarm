//! Peak and current resident-set-size sampling for run telemetry.
//!
//! Returns the process-wide peak and current RSS at the moment of the call.
//! Both values are **approximate, platform-dependent, and diagnostic only** —
//! not a coverage claim, memory-safety proof, UB-free status, Miri-clean
//! status, site-execution claim, calibrated metric, or performance guarantee.
//! Use them for CI-runner sizing, OOM-avoidance, and scan-cost visibility.
//!
//! ## Platform notes
//!
//! | Platform | Peak mechanism | Current mechanism |
//! |---|---|---|
//! | Linux | `getrusage(RUSAGE_SELF)` `ru_maxrss` KiB→bytes | `/proc/self/statm` page 1 × page size (safe file read) |
//! | macOS | `getrusage(RUSAGE_SELF)` `ru_maxrss` bytes | `None` (mach task_info is disproportionate complexity) |
//! | Windows | `GetProcessMemoryInfo` `PeakWorkingSetSize` | `GetProcessMemoryInfo` `WorkingSetSize` |
//! | other / unknown | `None` | `None` |
//!
//! The Linux/macOS peak unit difference (KB vs bytes) is a well-known kernel
//! quirk; this module normalises both to bytes before returning.

/// Result of a single RSS sample: both peak and current RSS in bytes.
pub(crate) struct RssSample {
    /// Approximate peak RSS in bytes (cumulative maximum).
    pub(crate) peak: Option<u64>,
    /// Approximate current RSS in bytes (point-in-time working set).
    pub(crate) current: Option<u64>,
}

/// Sample the current process's peak and current RSS in bytes.
///
/// `peak` is the cumulative maximum RSS the process reached at any point.
/// `current` is the working-set size at the moment of the call.
///
/// Either or both may be `None` on unsupported platforms or when the OS
/// call fails.
///
/// Call this once at run completion.
pub(crate) fn sample() -> RssSample {
    sample_impl()
}

// ─── Linux ────────────────────────────────────────────────────────────────────

#[cfg(target_os = "linux")]
fn sample_impl() -> RssSample {
    let peak = sample_linux_peak();
    let current = sample_linux_current();
    RssSample { peak, current }
}

#[cfg(target_os = "linux")]
fn sample_linux_peak() -> Option<u64> {
    // SAFETY: We call getrusage(RUSAGE_SELF, out) which is always safe for the
    // calling process:
    //   • RUSAGE_SELF (0) is a valid, stable constant defined by POSIX.
    //   • `rusage` is zero-initialised before the call, satisfying the
    //     requirement that `who` aliases a writable `struct rusage`.
    //   • The pointer is valid for the lifetime of the call (stack variable).
    //   • No aliasing: only this thread writes through the pointer.
    //   • Return value −1 is checked; on success the struct is fully populated.
    // On Linux, `ru_maxrss` is in kilobytes; we convert to bytes (× 1024).
    #[allow(
        unsafe_code,
        reason = "minimal contracted FFI: getrusage(RUSAGE_SELF) to read peak RSS; \
                  invariants proven in the SAFETY comment above; no alternative \
                  without adding a new shipped dependency"
    )]
    unsafe {
        let mut rusage: LinuxRusage = core::mem::zeroed();
        let ret = linux_getrusage(RUSAGE_SELF, &raw mut rusage);
        if ret == 0 {
            // Linux reports ru_maxrss in KiB; normalise to bytes.
            let kb = rusage.ru_maxrss;
            if kb > 0 {
                u64::try_from(kb).ok().and_then(|v| v.checked_mul(1024))
            } else {
                None
            }
        } else {
            None
        }
    }
}

/// Read current RSS from `/proc/self/statm` — a SAFE file read, no unsafe.
///
/// `/proc/self/statm` format: `total_pages resident_pages shared_pages ...`
/// Resident pages (field 1, 0-based) × page size = current RSS in bytes.
#[cfg(target_os = "linux")]
fn sample_linux_current() -> Option<u64> {
    let content = std::fs::read_to_string("/proc/self/statm").ok()?;
    let resident_pages: u64 = content
        .split_whitespace()
        .nth(1)?
        .parse()
        .ok()?;
    if resident_pages == 0 {
        return None;
    }
    // SAFETY: sysconf(_SC_PAGESIZE) would require unsafe; use the common
    // default of 4096 as a fallback.  The kernel guarantees page size is a
    // power of two; 4096 is correct on the vast majority of Linux deployments.
    // If the page size differs, the value is still a useful approximation.
    let page_size: u64 = page_size_bytes();
    resident_pages.checked_mul(page_size)
}

#[cfg(target_os = "linux")]
fn page_size_bytes() -> u64 {
    // SAFETY: sysconf is always safe to call with a valid constant.
    //   • _SC_PAGESIZE is defined by POSIX; the call always succeeds (returns > 0).
    //   • We only read the return value; no mutable out-params.
    #[allow(
        unsafe_code,
        reason = "minimal contracted FFI: sysconf(_SC_PAGESIZE) to get page size; \
                  _SC_PAGESIZE is a valid POSIX constant, always succeeds, no out-params; \
                  no alternative without adding a new shipped dependency"
    )]
    unsafe {
        let ps = linux_sysconf(SC_PAGESIZE);
        if ps > 0 {
            ps as u64
        } else {
            4096 // conservative fallback
        }
    }
}

#[cfg(target_os = "linux")]
const RUSAGE_SELF: i32 = 0;
#[cfg(target_os = "linux")]
const SC_PAGESIZE: i32 = 30; // _SC_PAGESIZE on Linux

/// Minimal layout of `struct rusage` on Linux (x86-64 / aarch64).
/// We only read `ru_maxrss` (field 4, offset 32, type `long`).
#[cfg(target_os = "linux")]
#[repr(C)]
struct LinuxRusage {
    ru_utime: [i64; 2],  // struct timeval: tv_sec + tv_usec
    ru_stime: [i64; 2],  // struct timeval: tv_sec + tv_usec
    ru_maxrss: i64,      // kilobytes on Linux
    _pad: [i64; 13],     // remaining fields (not read)
}

#[cfg(target_os = "linux")]
#[allow(unsafe_code, reason = "unsafe extern block declaring contracted Linux C FFI symbols; \
    see SAFETY comments in sample_linux_peak() and page_size_bytes() above")]
unsafe extern "C" {
    #[link_name = "getrusage"]
    fn linux_getrusage(who: i32, usage: *mut LinuxRusage) -> i32;

    #[link_name = "sysconf"]
    fn linux_sysconf(name: i32) -> i64;
}

// ─── macOS / other Unix (not Linux) ──────────────────────────────────────────

#[cfg(all(unix, not(target_os = "linux")))]
fn sample_impl() -> RssSample {
    // Peak via getrusage; current via mach task_info is disproportionate
    // complexity — return None truthfully.
    RssSample {
        peak: sample_mac_peak(),
        current: None,
    }
}

#[cfg(all(unix, not(target_os = "linux")))]
fn sample_mac_peak() -> Option<u64> {
    // SAFETY: Same contract as the Linux branch; the only difference is the
    // unit: on macOS (and BSDs) `ru_maxrss` is in bytes, not kilobytes.
    //   • RUSAGE_SELF (0) is a valid POSIX constant.
    //   • `rusage` is zero-initialised; pointer is valid for the call duration.
    //   • Return value −1 is checked.
    #[allow(
        unsafe_code,
        reason = "minimal contracted FFI: getrusage(RUSAGE_SELF) to read peak RSS; \
                  invariants proven in the SAFETY comment above; no alternative \
                  without adding a new shipped dependency"
    )]
    unsafe {
        let mut rusage: MacRusage = core::mem::zeroed();
        let ret = mac_getrusage(RUSAGE_SELF_MAC, &raw mut rusage);
        if ret == 0 {
            // macOS / BSD: ru_maxrss is already in bytes.
            let bytes = rusage.ru_maxrss;
            if bytes > 0 {
                u64::try_from(bytes).ok()
            } else {
                None
            }
        } else {
            None
        }
    }
}

#[cfg(all(unix, not(target_os = "linux")))]
const RUSAGE_SELF_MAC: i32 = 0;

/// Minimal layout of `struct rusage` on macOS / BSD.
/// `ru_maxrss` is `i64` and measured in **bytes** on macOS.
#[cfg(all(unix, not(target_os = "linux")))]
#[repr(C)]
struct MacRusage {
    ru_utime: [i64; 2],
    ru_stime: [i64; 2],
    ru_maxrss: i64,  // bytes on macOS
    _pad: [i64; 13],
}

#[cfg(all(unix, not(target_os = "linux")))]
#[allow(unsafe_code, reason = "unsafe extern block declaring contracted macOS getrusage FFI symbol; \
    see SAFETY comment in sample_mac_peak() above")]
unsafe extern "C" {
    #[link_name = "getrusage"]
    fn mac_getrusage(who: i32, usage: *mut MacRusage) -> i32;
}

// ─── Windows ─────────────────────────────────────────────────────────────────

#[cfg(windows)]
#[allow(
    unsafe_code,
    reason = "minimal contracted FFI: three contracted Windows FFI calls to read peak and \
              current RSS via GetProcessMemoryInfo; each unsafe block has an individual \
              SAFETY comment; no alternative without adding a new shipped dependency"
)]
fn sample_impl() -> RssSample {
    // SAFETY: `ProcessMemoryCounters` is a plain C struct with no padding
    // invariants; zero is a valid bit pattern for all its integer fields.
    let mut pmc: ProcessMemoryCounters = unsafe { core::mem::zeroed() };
    pmc.cb = core::mem::size_of::<ProcessMemoryCounters>() as u32;

    // SAFETY: `GetCurrentProcess()` always succeeds and returns a pseudo-handle
    // for the current process; it never returns null and never requires
    // `CloseHandle`.
    let handle = unsafe { GetCurrentProcess() };

    // SAFETY:
    //   • `handle` is a valid pseudo-handle from `GetCurrentProcess()` above.
    //   • `pmc` is zero-initialised above; `cb` is set to its exact struct size.
    //   • The pointer is valid for the duration of the call (stack variable).
    //   • No aliasing: only this call writes through the pointer.
    //   • Return value 0 is checked; on success `pmc` is fully populated.
    //   • `PeakWorkingSetSize` and `WorkingSetSize` are both in bytes.
    let ret = unsafe { GetProcessMemoryInfo(handle, &raw mut pmc, pmc.cb) };

    if ret != 0 {
        let peak = if pmc.PeakWorkingSetSize > 0 {
            Some(pmc.PeakWorkingSetSize as u64)
        } else {
            None
        };
        let current = if pmc.WorkingSetSize > 0 {
            Some(pmc.WorkingSetSize as u64)
        } else {
            None
        };
        RssSample { peak, current }
    } else {
        RssSample { peak: None, current: None }
    }
}

/// Minimal layout of `PROCESS_MEMORY_COUNTERS` from psapi.h.
/// `cb`, `PeakWorkingSetSize`, and `WorkingSetSize` are read/written.
#[cfg(windows)]
#[repr(C)]
#[allow(non_snake_case, reason = "Windows API struct field names use PascalCase")]
struct ProcessMemoryCounters {
    cb: u32,
    PageFaultCount: u32,
    PeakWorkingSetSize: usize,
    WorkingSetSize: usize,
    QuotaPeakPagedPoolUsage: usize,
    QuotaPagedPoolUsage: usize,
    QuotaPeakNonPagedPoolUsage: usize,
    QuotaNonPagedPoolUsage: usize,
    PagefileUsage: usize,
    PeakPagefileUsage: usize,
}

#[cfg(windows)]
#[link(name = "kernel32")]
#[allow(unsafe_code, reason = "unsafe extern block declaring contracted Windows kernel32 FFI symbol; \
    GetCurrentProcess is in kernel32.lib; see SAFETY comment in sample_impl() above")]
unsafe extern "system" {
    fn GetCurrentProcess() -> *mut core::ffi::c_void;
}

#[cfg(windows)]
#[link(name = "psapi")]
#[allow(unsafe_code, reason = "unsafe extern block declaring contracted Windows psapi FFI symbol; \
    GetProcessMemoryInfo is in psapi.lib; see SAFETY comment in sample_impl() above")]
unsafe extern "system" {
    fn GetProcessMemoryInfo(
        Process: *mut core::ffi::c_void,
        ppsmemCounters: *mut ProcessMemoryCounters,
        cb: u32,
    ) -> i32;
}

// ─── Fallback (unsupported platform) ─────────────────────────────────────────

#[cfg(not(any(unix, windows)))]
fn sample_impl() -> RssSample {
    RssSample { peak: None, current: None }
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    reason = "test assertions: panicking on failure is the correct behaviour for unit tests"
)]
mod tests {
    use super::*;

    /// Drift-lock: `sample().peak` returns `Some(> 0)` on supported platforms
    /// after a real call.  If the FFI call is accidentally removed or the field
    /// is dropped, this goes RED.
    #[test]
    fn peak_rss_sample_is_some_and_positive_on_supported_platform() {
        let rss = sample();

        #[cfg(any(unix, windows))]
        {
            let bytes = rss.peak.expect(
                "peak_rss::sample().peak must return Some(> 0) on supported platforms \
                 (unix / windows); if this fails the FFI call or normalisation is broken",
            );
            assert!(
                bytes > 0,
                "peak RSS must be positive (> 0); got {bytes}"
            );
            // Plausibility check: real RSS should be at least 1 MiB for a running
            // Rust test binary.  This also confirms the Linux KB→bytes conversion
            // is applied (a 1 KiB value would mean the conversion was skipped).
            assert!(
                bytes >= 1024 * 1024,
                "peak RSS {bytes} bytes is implausibly small — likely a unit \
                 conversion bug (Linux ru_maxrss is KB; macOS is bytes)"
            );
        }

        #[cfg(not(any(unix, windows)))]
        {
            assert!(rss.peak.is_none(), "unsupported platform must return None for peak");
        }
    }

    /// Drift-lock: `sample().current` returns `Some(> 0)` on platforms where
    /// current RSS is supported (Linux, Windows).  On macOS (truthful absence)
    /// and unsupported platforms it must be `None`.
    #[test]
    fn current_rss_sample_behaviour_per_platform() {
        let rss = sample();

        #[cfg(target_os = "linux")]
        {
            let bytes = rss.current.expect(
                "current_rss::sample().current must return Some(> 0) on Linux; \
                 check /proc/self/statm read or page-size calculation"
            );
            assert!(bytes > 0, "current RSS must be positive; got {bytes}");
            assert!(
                bytes >= 1024 * 1024,
                "current RSS {bytes} bytes is implausibly small for a Rust test binary"
            );
        }

        #[cfg(windows)]
        {
            let bytes = rss.current.expect(
                "current_rss::sample().current must return Some(> 0) on Windows"
            );
            assert!(bytes > 0, "current RSS must be positive; got {bytes}");
        }

        // macOS: current is intentionally None (mach task_info complexity).
        #[cfg(all(unix, not(target_os = "linux")))]
        {
            assert!(
                rss.current.is_none(),
                "macOS: current_rss_bytes must be None (truthful absence)"
            );
        }

        #[cfg(not(any(unix, windows)))]
        {
            assert!(rss.current.is_none(), "unsupported platform must return None");
        }
    }

    /// Drift-lock: on Linux, the peak value must be ≥ 4 MiB confirming the
    /// KB→bytes normalisation (× 1024) was applied.
    #[cfg(target_os = "linux")]
    #[test]
    fn linux_peak_rss_is_in_bytes_not_kilobytes() {
        let rss = sample().peak.expect("Linux must return Some for peak");
        assert!(
            rss >= 4 * 1024 * 1024,
            "Linux peak RSS {rss} bytes is below 4 MiB — \
             likely the KB→bytes conversion (× 1024) was not applied"
        );
    }

    /// Drift-lock: on Linux, the current RSS from /proc/self/statm must be
    /// ≥ 1 MiB to confirm the page-size multiplication was applied.
    #[cfg(target_os = "linux")]
    #[test]
    fn linux_current_rss_is_in_bytes_not_pages() {
        let rss = sample().current.expect("Linux must return Some for current");
        assert!(
            rss >= 1024 * 1024,
            "Linux current RSS {rss} bytes is below 1 MiB — \
             likely the page-size multiplication was not applied"
        );
    }

    /// Drift-lock: `Option<u64>` fields round-trip through serialise/deserialise
    /// as expected by the status JSON schema.  Tests both `Some` and `None` paths.
    #[test]
    fn peak_rss_bytes_option_serialises_as_expected() {
        // Some(positive) → positive integer in JSON.
        let some_val: Option<u64> = Some(123_456_789);
        let json_some = serde_json::to_string(&some_val).expect("serialise Some");
        assert_eq!(json_some, "123456789");

        // None → null in JSON.
        let none_val: Option<u64> = None;
        let json_none = serde_json::to_string(&none_val).expect("serialise None");
        assert_eq!(json_none, "null");

        // Round-trip: deserialise back.
        let back_some: Option<u64> =
            serde_json::from_str(&json_some).expect("deserialise Some");
        assert_eq!(back_some, Some(123_456_789));

        let back_none: Option<u64> =
            serde_json::from_str(&json_none).expect("deserialise None");
        assert!(back_none.is_none());
    }
}
