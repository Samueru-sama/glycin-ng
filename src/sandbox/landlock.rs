//! Filesystem-restriction layer using Linux landlock.

use crate::LandlockPosture;

#[cfg(all(target_os = "linux", feature = "landlock"))]
pub(crate) fn apply() -> LandlockPosture {
    use landlock::{ABI, Access, AccessFs, Ruleset, RulesetAttr, RulesetStatus};

    let abi = ABI::V1;
    let access_all = AccessFs::from_all(abi);

    let ruleset = match Ruleset::default().handle_access(access_all) {
        Ok(r) => r,
        Err(_) => {
            return LandlockPosture::Unsupported {
                reason: "handle_access failed",
            };
        }
    };
    let created = match ruleset.create() {
        Ok(c) => c,
        Err(_) => {
            return LandlockPosture::Unsupported {
                reason: "ruleset create failed",
            };
        }
    };
    let status = match created.restrict_self() {
        Ok(s) => s,
        Err(_) => {
            return LandlockPosture::Unsupported {
                reason: "restrict_self failed",
            };
        }
    };

    match status.ruleset {
        RulesetStatus::FullyEnforced | RulesetStatus::PartiallyEnforced => {
            LandlockPosture::Enforced { abi: abi as u32 }
        }
        RulesetStatus::NotEnforced => LandlockPosture::Unsupported {
            reason: "kernel does not support landlock",
        },
    }
}

#[cfg(not(all(target_os = "linux", feature = "landlock")))]
pub(crate) fn apply() -> LandlockPosture {
    LandlockPosture::Disabled
}
