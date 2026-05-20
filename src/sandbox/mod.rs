//! Sandbox primitives and posture reporting.
//!
//! The actual enforcement implementations live in submodules behind
//! their feature gates. The types in this module are always
//! available so callers can write portable code that observes
//! posture without conditionally compiling.

/// Result of applying every requested sandbox layer for one decode.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct SandboxPosture {
    /// Filesystem restrictions, see [`LandlockPosture`].
    pub landlock: LandlockPosture,
    /// Syscall filter, see [`SeccompPosture`].
    pub seccomp: SeccompPosture,
    /// Per-process resource caps, see [`RlimitPosture`].
    pub rlimit: RlimitPosture,
}

impl SandboxPosture {
    /// Posture in which no layer is active. Used when sandbox features
    /// are off at build time or the platform lacks support.
    pub const fn none() -> Self {
        Self {
            landlock: LandlockPosture::Disabled,
            seccomp: SeccompPosture::Disabled,
            rlimit: RlimitPosture::Disabled,
        }
    }

    /// Whether every layer reports as actively enforcing restrictions.
    pub fn is_fully_enforced(self) -> bool {
        matches!(self.landlock, LandlockPosture::Enforced { .. })
            && matches!(self.seccomp, SeccompPosture::Enforced)
            && matches!(self.rlimit, RlimitPosture::Applied { .. })
    }
}

impl Default for SandboxPosture {
    fn default() -> Self {
        Self::none()
    }
}

/// Filesystem-restriction layer status.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum LandlockPosture {
    /// Landlock was applied; the value is the ruleset ABI version
    /// negotiated with the kernel.
    Enforced {
        /// Negotiated ABI version.
        abi: u32,
    },
    /// Kernel does not support landlock or rejected the ruleset.
    Unsupported {
        /// Short reason string for logging.
        reason: &'static str,
    },
    /// The `landlock` Cargo feature was off, or the loader was asked
    /// to skip this layer.
    Disabled,
}

/// Syscall-filter layer status.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SeccompPosture {
    /// Filter program installed on the decode thread.
    Enforced,
    /// Kernel rejected the filter or the platform does not support
    /// seccomp.
    Unsupported {
        /// Short reason string for logging.
        reason: &'static str,
    },
    /// The `seccomp` Cargo feature was off, or the loader was asked
    /// to skip this layer.
    Disabled,
}

/// `setrlimit` layer status.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum RlimitPosture {
    /// Both `RLIMIT_AS` and `RLIMIT_CPU` applied.
    Applied {
        /// `RLIMIT_AS` cap in MiB.
        as_mib: u64,
        /// `RLIMIT_CPU` cap in seconds.
        cpu_seconds: u64,
    },
    /// One of the two limits could not be set.
    PartiallyApplied {
        /// Short reason string for logging.
        detail: &'static str,
    },
    /// The loader was asked to skip this layer.
    Disabled,
}

/// Caller-side selection of which sandbox layers to attempt.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct SandboxSelector {
    /// Attempt landlock.
    pub landlock: bool,
    /// Attempt seccomp.
    pub seccomp: bool,
    /// Apply `RLIMIT_AS` and `RLIMIT_CPU` from
    /// [`Limits`](crate::Limits).
    pub rlimit: bool,
    /// Refuse the decode if any selected layer fails to apply.
    pub strict: bool,
}

impl Default for SandboxSelector {
    fn default() -> Self {
        Self {
            landlock: true,
            seccomp: true,
            rlimit: true,
            strict: false,
        }
    }
}

impl SandboxSelector {
    /// No sandbox at all.
    pub const fn none() -> Self {
        Self {
            landlock: false,
            seccomp: false,
            rlimit: false,
            strict: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn none_is_not_fully_enforced() {
        assert!(!SandboxPosture::none().is_fully_enforced());
    }

    #[test]
    fn enforced_posture_reports_fully_enforced() {
        let p = SandboxPosture {
            landlock: LandlockPosture::Enforced { abi: 5 },
            seccomp: SeccompPosture::Enforced,
            rlimit: RlimitPosture::Applied {
                as_mib: 512,
                cpu_seconds: 30,
            },
        };
        assert!(p.is_fully_enforced());
    }

    #[test]
    fn one_layer_disabled_breaks_full_enforcement() {
        let p = SandboxPosture {
            landlock: LandlockPosture::Disabled,
            seccomp: SeccompPosture::Enforced,
            rlimit: RlimitPosture::Applied {
                as_mib: 512,
                cpu_seconds: 30,
            },
        };
        assert!(!p.is_fully_enforced());
    }

    #[test]
    fn selector_default_attempts_every_layer() {
        let s = SandboxSelector::default();
        assert!(s.landlock);
        assert!(s.seccomp);
        assert!(s.rlimit);
        assert!(!s.strict);
    }

    #[test]
    fn selector_none_disables_every_layer() {
        let s = SandboxSelector::none();
        assert!(!s.landlock);
        assert!(!s.seccomp);
        assert!(!s.rlimit);
    }
}
