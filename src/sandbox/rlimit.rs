//! Process-wide memory and CPU caps via `setrlimit`.
//!
//! Both `RLIMIT_AS` and `RLIMIT_CPU` are process-wide on Linux, so
//! these caps apply to the entire host process, not just the decode
//! worker thread. Callers who care should hold the cap at a value
//! that does not break their main thread, or disable this layer
//! entirely via [`SandboxSelector::rlimit`](crate::SandboxSelector).

use crate::RlimitPosture;

#[cfg(target_os = "linux")]
pub(crate) fn apply(decode_memory_mib: u64, decode_cpu_seconds: u64) -> RlimitPosture {
    let as_bytes = decode_memory_mib.saturating_mul(1024 * 1024);
    let as_limit = libc::rlimit {
        rlim_cur: as_bytes as libc::rlim_t,
        rlim_max: as_bytes as libc::rlim_t,
    };
    let cpu_limit = libc::rlimit {
        rlim_cur: decode_cpu_seconds as libc::rlim_t,
        rlim_max: decode_cpu_seconds as libc::rlim_t,
    };

    // SAFETY: setrlimit is async-signal-safe and may be called from
    // any thread. The struct pointers are valid for the call.
    let as_rc = unsafe { libc::setrlimit(libc::RLIMIT_AS, &as_limit) };
    let cpu_rc = unsafe { libc::setrlimit(libc::RLIMIT_CPU, &cpu_limit) };

    match (as_rc, cpu_rc) {
        (0, 0) => RlimitPosture::Applied {
            as_mib: decode_memory_mib,
            cpu_seconds: decode_cpu_seconds,
        },
        (0, _) => RlimitPosture::PartiallyApplied {
            detail: "RLIMIT_CPU not applied",
        },
        (_, 0) => RlimitPosture::PartiallyApplied {
            detail: "RLIMIT_AS not applied",
        },
        _ => RlimitPosture::PartiallyApplied {
            detail: "neither RLIMIT_AS nor RLIMIT_CPU applied",
        },
    }
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn apply(_decode_memory_mib: u64, _decode_cpu_seconds: u64) -> RlimitPosture {
    RlimitPosture::Disabled
}
