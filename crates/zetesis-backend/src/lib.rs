//! The backend vocabulary every zetesis tool shares: how a program runs.
//!
//! A request is [`Backend::Cpu`], the default, or [`Backend::Gpu`] with an
//! optional [`GpuApi`]. `Gpu(None)` asks for the platform's native API; the one
//! rule that makes it concrete is [`Backend::resolved_api`], applied where a GPU
//! is opened, so code past that point never holds an unresolved request.
//! Choosing one GPU among several is a device choice, not a backend, and is not
//! part of this vocabulary.
//!
//! The vocabulary does not depend on whether GPU support is compiled: a build
//! without it still reads every value and refuses a GPU request with its reason,
//! never with "unknown value". Spellings are stable — they appear in arguments,
//! configuration and retained records — and a spelling zetesis no longer accepts
//! is answered with what to use instead ([`ParseBackendError::retired`]).
//!
//! The CPU backend's default parallelism is here too ([`default_threads`]), so
//! every tool that runs or measures zetesis means the same count by `auto`.
#![forbid(unsafe_code)]

use std::fmt;
use std::num::NonZeroUsize;
use std::str::FromStr;

/// The CPU backend's default thread count, which `--threads auto` means: the
/// host's available parallelism, at most four, or one when the host reports
/// none. It reads the host each time it is called.
#[must_use]
pub fn default_threads() -> NonZeroUsize {
    const CAP: NonZeroUsize = NonZeroUsize::new(4).expect("four is nonzero");
    std::thread::available_parallelism()
        .unwrap_or(NonZeroUsize::MIN)
        .min(CAP)
}

/// Read a thread count as a command line spells it: `auto` for
/// [`default_threads`], or a positive count.
///
/// # Errors
/// Refuses any other spelling, naming the accepted forms.
pub fn parse_threads(value: &str) -> Result<NonZeroUsize, String> {
    if value == "auto" {
        Ok(default_threads())
    } else {
        value
            .parse()
            .map_err(|_| "expected auto or a positive thread count".to_owned())
    }
}

/// How a program runs: its execution backend.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Backend {
    /// CPU execution, the default.
    #[default]
    Cpu,
    /// A GPU through the named API, or through the platform's native API when
    /// none is named.
    Gpu(Option<GpuApi>),
}

/// A GPU compute API zetesis targets.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GpuApi {
    /// Metal, on macOS.
    Metal,
    /// Vulkan, on Linux and on other platforms with a Vulkan driver.
    Vulkan,
}

impl GpuApi {
    /// The platform's native API, defined for every target: Metal on Apple
    /// targets and Vulkan on all others, the APIs zetesis compiles there.
    #[must_use]
    pub const fn native() -> Self {
        if cfg!(target_vendor = "apple") {
            Self::Metal
        } else {
            Self::Vulkan
        }
    }

    /// Stable spelling, as `--backend` takes it.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Metal => "metal",
            Self::Vulkan => "vulkan",
        }
    }

    /// The API's proper name, for messages.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Metal => "Metal",
            Self::Vulkan => "Vulkan",
        }
    }
}

impl Backend {
    /// Every value, in the order help and documentation list them.
    pub const ALL: [Self; 4] = [
        Self::Cpu,
        Self::Gpu(None),
        Self::Gpu(Some(GpuApi::Metal)),
        Self::Gpu(Some(GpuApi::Vulkan)),
    ];

    /// Stable spelling for arguments, configuration and records.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Cpu => "cpu",
            Self::Gpu(None) => "gpu",
            Self::Gpu(Some(api)) => api.label(),
        }
    }

    /// One line of help for the value, as command help shows it.
    #[must_use]
    pub const fn help(self) -> &'static str {
        match self {
            Self::Cpu => "CPU execution (the default)",
            Self::Gpu(None) => {
                "A GPU through the platform's native API: Metal on macOS, Vulkan elsewhere"
            }
            Self::Gpu(Some(GpuApi::Metal)) => "A GPU through Metal (macOS)",
            Self::Gpu(Some(GpuApi::Vulkan)) => "A GPU through Vulkan (Linux)",
        }
    }

    /// Whether the request is for a GPU.
    #[must_use]
    pub const fn is_gpu(self) -> bool {
        matches!(self, Self::Gpu(_))
    }

    /// The API a GPU request opens on this platform — the named one, or the
    /// native one when none is named — and `None` for the CPU. This is the one
    /// resolution rule; each consumer applies it at the boundary where it opens
    /// a GPU.
    #[must_use]
    pub const fn resolved_api(self) -> Option<GpuApi> {
        match self {
            Self::Cpu => None,
            Self::Gpu(Some(api)) => Some(api),
            Self::Gpu(None) => Some(GpuApi::native()),
        }
    }
}

impl fmt::Display for Backend {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.label())
    }
}

impl fmt::Display for GpuApi {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.label())
    }
}

/// Spellings zetesis once accepted, each with what to use instead.
const RETIRED: [(&str, &str); 4] = [
    (
        "auto",
        "CPU is the default; `auto` returns when zetesis can choose a backend for each problem",
    ),
    (
        "dx12",
        "zetesis targets Metal on macOS and Vulkan on Linux; use gpu, metal or vulkan",
    ),
    (
        "gl",
        "zetesis targets Metal on macOS and Vulkan on Linux; use gpu, metal or vulkan",
    ),
    (
        "nvidia",
        "use gpu or vulkan; choosing one GPU among several will come with NVIDIA qualification",
    ),
];

/// A spelling that is not a backend.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseBackendError {
    spelling: String,
    retired: Option<&'static str>,
}

impl ParseBackendError {
    /// The spelling that was given.
    #[must_use]
    pub fn spelling(&self) -> &str {
        &self.spelling
    }

    /// Why a spelling zetesis once accepted no longer is, and what to use
    /// instead; `None` for a spelling it never accepted.
    #[must_use]
    pub const fn retired(&self) -> Option<&'static str> {
        self.retired
    }
}

impl fmt::Display for ParseBackendError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.retired {
            Some(reason) => write!(
                formatter,
                "`{}` is no longer a backend: {reason}",
                self.spelling
            ),
            None => write!(
                formatter,
                "`{}` is not a backend; use cpu, gpu, metal or vulkan",
                self.spelling
            ),
        }
    }
}

impl std::error::Error for ParseBackendError {}

impl FromStr for Backend {
    type Err = ParseBackendError;

    fn from_str(spelling: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|backend| backend.label() == spelling)
            .ok_or_else(|| ParseBackendError {
                spelling: spelling.to_owned(),
                retired: RETIRED
                    .iter()
                    .find(|(old, _)| *old == spelling)
                    .map(|(_, reason)| *reason),
            })
    }
}

/// Command-line parsing for [`Backend`] with clap's standard behaviour — help
/// lists the four values with their meaning, an invalid value is refused with
/// the possible values and a suggestion for a near miss, and `ignore_case` is
/// honoured — except that a retired spelling is answered with what to use
/// instead rather than rejected bare.
#[cfg(feature = "clap")]
#[derive(Clone, Copy, Debug, Default)]
pub struct BackendParser;

#[cfg(feature = "clap")]
impl clap::builder::TypedValueParser for BackendParser {
    type Value = Backend;

    fn parse_ref(
        &self,
        command: &clap::Command,
        argument: Option<&clap::Arg>,
        value: &std::ffi::OsStr,
    ) -> Result<Backend, clap::Error> {
        let spelling = clap::builder::PossibleValuesParser::new(possible_values())
            .parse_ref(command, argument, value)
            .map_err(|refusal| retired(command, argument, value).unwrap_or(refusal))?;
        // The standard parser accepted a label, in any ASCII case when the
        // argument ignores case; the labels themselves are lowercase ASCII.
        spelling
            .to_ascii_lowercase()
            .parse()
            .map_err(|error| invalid(command, argument, &error))
    }

    fn possible_values(
        &self,
    ) -> Option<Box<dyn Iterator<Item = clap::builder::PossibleValue> + '_>> {
        Some(Box::new(possible_values()))
    }
}

#[cfg(feature = "clap")]
fn possible_values() -> impl Iterator<Item = clap::builder::PossibleValue> {
    Backend::ALL
        .into_iter()
        .map(|backend| clap::builder::PossibleValue::new(backend.label()).help(backend.help()))
}

/// The explanation of a retired spelling, as a command-line refusal; `None`
/// for any other value.
#[cfg(feature = "clap")]
fn retired(
    command: &clap::Command,
    argument: Option<&clap::Arg>,
    value: &std::ffi::OsStr,
) -> Option<clap::Error> {
    let spelling = value.to_str()?;
    let ignore_case = argument.is_some_and(clap::Arg::is_ignore_case_set);
    let (_, reason) = RETIRED.iter().find(|(old, _)| {
        if ignore_case {
            old.eq_ignore_ascii_case(spelling)
        } else {
            *old == spelling
        }
    })?;
    let error = ParseBackendError {
        spelling: spelling.to_owned(),
        retired: Some(reason),
    };
    Some(invalid(command, argument, &error))
}

#[cfg(feature = "clap")]
fn invalid(
    command: &clap::Command,
    argument: Option<&clap::Arg>,
    error: &ParseBackendError,
) -> clap::Error {
    let name = argument.map_or_else(|| "the backend".to_owned(), ToString::to_string);
    clap::Error::raw(
        clap::error::ErrorKind::InvalidValue,
        format!(
            "invalid value '{}' for '{name}': {error}\n",
            error.spelling()
        ),
    )
    .with_cmd(command)
}

/// Records store the stable spelling.
#[cfg(feature = "serde")]
impl serde::Serialize for Backend {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.label())
    }
}

#[cfg(test)]
mod tests;
