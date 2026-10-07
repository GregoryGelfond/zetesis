use std::ffi::OsString;

use crate::selected::NativeExecution;

/// Native executable interface, independent of the measured execution profile.
/// No probing, fallback or solver heuristic change is performed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NativeInvocation {
    /// Historical flat spelling. The requested policy remains ordinary public
    /// controls; older executables retain their own effective defaults.
    #[default]
    Legacy,
    /// Explicit `zetesis solve --backend … --threads …` interface.
    Solve,
}

impl NativeInvocation {
    /// Arguments requesting the same complete native profile through the chosen
    /// interface. The caller appends machine output flags and one source path.
    #[must_use]
    pub fn arguments(self, execution: &NativeExecution) -> Vec<OsString> {
        let mut arguments = execution.arguments();
        if self == Self::Legacy {
            for argument in &mut arguments {
                if argument == "--threads" {
                    *argument = "--workers".into();
                }
            }
        } else {
            // The profile always requests complete enumeration. The explicit
            // command has one canonical spelling for that semantic selection.
            if let Some(models) = arguments.iter().position(|value| value == "--models") {
                arguments.splice(models..models + 2, ["--all".into()]);
            }
            arguments.insert(0, "solve".into());
        }
        arguments
    }

    pub(super) fn help_arguments(self) -> Vec<OsString> {
        match self {
            Self::Legacy => vec!["--help-all".into()],
            Self::Solve => vec!["solve".into(), "--help-all".into()],
        }
    }
}
