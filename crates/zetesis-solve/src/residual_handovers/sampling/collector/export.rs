//! Stream the original, independently sampled populations before terminal rank selection.
use super::{Batch, Query, RunSamples, Sampler, World};
use std::fs::File;
use std::io::{self, Seek, SeekFrom, Write};
use zetesis_backend::residual_evidence as wire;
use zetesis_wgpu::ResidualReason;

pub(super) enum Export {
    Pending {
        file: File,
        nonce: [u8; 16],
        max_bytes: u64,
    },
    Written(io::Result<Written>),
    Consumed,
}

impl RunSamples {
    pub(crate) fn offer(&mut self, file: File, nonce: [u8; 16], max_bytes: u64) -> io::Result<()> {
        if !self.export.is_empty() || self.state[0].settled.is_some() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "sample export already offered or run settled",
            ));
        }
        let metadata = file.metadata()?;
        if max_bytes > wire::MAX_BYTES || !metadata.is_file() || metadata.len() != 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid sample allowance or nonempty evidence file",
            ));
        }
        self.export.try_reserve_exact(1).map_err(io::Error::other)?;
        self.export.push(Export::Pending {
            file,
            nonce,
            max_bytes,
        });
        Ok(())
    }

    pub(crate) fn take_export(&mut self) -> Option<io::Result<Written>> {
        let entry = self.export.first_mut()?;
        if matches!(entry, Export::Written(_)) {
            let Export::Written(result) = std::mem::replace(entry, Export::Consumed) else {
                unreachable!()
            };
            Some(result)
        } else {
            None
        }
    }
}

impl Export {
    pub(super) fn write(&mut self, sampler: &Sampler) {
        if let Self::Pending {
            mut file,
            nonce,
            max_bytes,
        } = std::mem::replace(self, Self::Consumed)
        {
            *self = Self::Written(
                write_records(&mut file, nonce, max_bytes, sampler)
                    .map(|header| Written { file, header }),
            );
        }
    }
}

/// The native session supplies its final semantic coverage before publication.
#[derive(Debug)]
pub(crate) struct Written {
    file: File,
    header: wire::Header,
}

impl Written {
    pub(crate) fn seal(mut self, complete: bool) -> io::Result<wire::Header> {
        if Some(self.file.metadata()?.len()) != self.header.byte_len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "staged residual artifact length changed",
            ));
        }
        self.header.complete = complete;
        self.file.rewind()?;
        self.file.write_all(&self.header.encode())?;
        self.file.flush()?;
        if Some(self.file.metadata()?.len()) != self.header.byte_len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "sealing changed residual artifact length",
            ));
        }
        Ok(self.header)
    }
}

/// Only the ready receipt can seal the header. A failed or interrupted stream
/// never leaves a valid header followed by truncated or stale data.
fn write_records(
    file: &mut File,
    nonce: [u8; 16],
    max_bytes: u64,
    sampler: &Sampler,
) -> io::Result<wire::Header> {
    let worlds = sampler.worlds.population();
    let batches = sampler.batches.population();
    let header = wire::Header {
        nonce,
        seed: sampler.seed,
        worlds: worlds.seen,
        batches: batches.seen,
        retained_worlds: u32::try_from(worlds.retained).map_err(io::Error::other)?,
        retained_batches: u32::try_from(batches.retained).map_err(io::Error::other)?,
        complete: false,
    };
    let size = header
        .byte_len()
        .filter(|n| *n <= max_bytes)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "residual artifact exceeds its byte allowance",
            )
        })?;
    // A regular file opened with append mode accepts seeks but ignores them
    // on write. Detect that before reserving the whole header: even the
    // smallest admissible artifact can accommodate this two-byte probe.
    file.set_len(1)?;
    file.rewind()?;
    file.write_all(&[0])?;
    if file.stream_position()? != 1 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "residual evidence file requires overwrite access",
        ));
    }
    file.seek(SeekFrom::Start(0))?;
    let mut output = LimitedFile {
        file,
        remaining: size,
    };
    output.record(&[0; 128])?;
    for &world in &sampler.worlds.records {
        output.record(&world_record(world).encode())?;
    }
    for &batch in &sampler.batches.records {
        output.record(&batch_record(batch).encode())?;
    }
    debug_assert_eq!(output.remaining, 0);
    output.file.flush()?;
    Ok(header)
}

struct LimitedFile<'a> {
    file: &'a mut File,
    remaining: u64,
}

impl LimitedFile<'_> {
    fn record(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.remaining = self
            .remaining
            .checked_sub(bytes.len() as u64)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "sample record exceeded declared length",
                )
            })?;
        self.file.write_all(bytes)
    }
}

fn world_record(world: World) -> wire::World {
    wire::World {
        reason: world.reason.map(|r| match r {
            ResidualReason::FixedPoint => wire::Reason::FixedPoint,
            ResidualReason::RoundLimit => wire::Reason::RoundLimit,
            ResidualReason::WorkLimit => wire::Reason::WorkLimit,
        }),
        open: world.open,
        query: match world.query {
            Query::Unknown => wire::Query::Unknown,
            Query::Certified => wire::Query::Certified,
            Query::Entered(q) => wire::Query::Entered {
                visited: q.visited as u64,
                attempted_splits: q.attempted_splits as u64,
                published_splits: q.published_splits as u64,
                depth: q.deepest.map(|d| d as u64),
                work: q.work,
                host_ns: q.host_time.map(|t| t.as_nanos()),
            },
        },
    }
}

fn batch_record(batch: Batch) -> wire::Batch {
    wire::Batch {
        residuals: batch.residuals,
        width: batch.width,
        host_ns: batch.times.map(|value| value.map(|t| t.as_nanos())),
    }
}
