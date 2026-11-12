//! Selection provenance shares the existing manifest accounting and snapshots.

use super::{Error, Workload};
use crate::selected::FileSeal;

impl Workload {
    pub(super) fn retained_manifest_bytes(&self, seal: &FileSeal) -> Result<usize, Error> {
        let mut metadata = self.metadata_bytes;
        for count in [
            seal.requested().as_os_str().len(),
            seal.canonical().as_os_str().len(),
            seal.sha256().len(),
        ] {
            metadata = super::add(metadata, count)?;
        }
        Ok(metadata)
    }

    pub(in crate::performance) fn attach_selection_manifest(
        &mut self,
        seal: &FileSeal,
        limit: usize,
    ) -> Result<(), Error> {
        let metadata = self.retained_manifest_bytes(seal)?;
        if metadata > limit {
            return Err(Error::Configuration(
                "selection manifest seal exceeds metadata ceiling",
            ));
        }
        self.selection_manifest = Some(seal.clone());
        self.metadata_bytes = metadata;
        Ok(())
    }

    pub(super) fn validate_selection_manifest(&self, limit: usize) -> Result<(), Error> {
        if let Some(seal) = &self.selection_manifest {
            let current =
                super::super::super::run::checked_seal(seal.requested(), limit, seal.sha256())?;
            if !current.same_identity(seal) {
                return Err(Error::Configuration(
                    "selection manifest file identity changed",
                ));
            }
        }
        Ok(())
    }
}
