//! One typed process exit, projected into the compatible corpus report fields.

use crate::process;
use serde::{Serialize, Serializer};

#[derive(Debug)]
pub(super) struct ExitEvidence(pub(super) Option<process::Exit>);

impl Serialize for ExitEvidence {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        struct View {
            exit_code: Option<i32>,
            exit_signal: Option<i32>,
        }
        View {
            exit_code: self.0.and_then(|exit| exit.code),
            exit_signal: self.0.and_then(|exit| exit.signal),
        }
        .serialize(serializer)
    }
}
