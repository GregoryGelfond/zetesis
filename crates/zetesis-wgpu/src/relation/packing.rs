use super::{
    BITS_PER_WORD, PARAM_BYTES, RECEIPT_MARKER, RECEIPT_WORDS, ROWS_PER_GROUP, RelationGpuLimits,
    RelationGpuMasks, capacity, poll,
};
use crate::{GpuError, GpuErrorKind};
use zetesis_core::relation::{Query, Relation};
use zetesis_cpu::Control;

const CONTROL_INTERVAL: usize = 1024;

pub(super) fn vector<T>(length: usize) -> Result<Vec<T>, GpuError> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(length)
        .map_err(|_| GpuError::new(GpuErrorKind::Allocation, "relation host reservation failed"))?;
    if values.capacity() > length {
        return Err(capacity("relation allocation exceeds planned capacity"));
    }
    Ok(values)
}

fn address(value: usize) -> Result<u32, GpuError> {
    u32::try_from(value).map_err(|_| capacity("relation dimension exceeds u32"))
}

fn cells(left: u32, right: u32) -> Result<u32, GpuError> {
    left.checked_mul(right)
        .ok_or_else(|| capacity("relation cell offset exceeds u32"))
}

fn host_words(bytes: u64) -> Result<usize, GpuError> {
    usize::try_from(bytes / 4).map_err(|_| capacity("relation buffer exceeds host indexing"))
}

pub(super) fn buffer(bytes: u64, limits: &wgpu::Limits) -> Result<(), GpuError> {
    if bytes > limits.max_buffer_size || bytes > limits.max_storage_buffer_binding_size {
        Err(capacity("relation buffer exceeds granted device limits"))
    } else {
        Ok(())
    }
}

pub(super) fn column_bytes(
    relation: &Relation<'_>,
    limits: &wgpu::Limits,
) -> Result<u64, GpuError> {
    let rows = address(relation.row_count())?;
    let columns = address(relation.predicate().arity())?;
    if rows > u32::MAX - ROWS_PER_GROUP {
        return Err(capacity("relation padded row address exceeds u32"));
    }
    let count = cells(rows, columns)?;
    if usize::try_from(count).ok() != Some(relation.columns().len()) {
        return Err(capacity("relation columns disagree with row/arity shape"));
    }
    let bytes = u64::from(count.max(1)) * 4;
    buffer(bytes, limits)?;
    Ok(bytes)
}

pub(super) struct Plan {
    pub(super) rows: u32,
    pub(super) queries: u32,
    pub(super) words: u32,
    pub(super) workgroups: [u32; 3],
    pub(super) column_bytes: u64,
    pub(super) query_bytes: u64,
    pub(super) equality_bytes: u64,
    pub(super) result_bytes: u64,
    pub(super) mask_bytes: u64,
    pub(super) transport_bytes: u64,
    pub(super) accounted_bytes: u64,
    pub(super) work: u64,
    columns: u32,
    epoch: u32,
}

impl Plan {
    pub(super) fn new(
        relation: &Relation<'_>,
        queries: &[Query<'_, '_>],
        limits: RelationGpuLimits,
        device: &wgpu::Limits,
        epoch: u32,
    ) -> Result<Self, GpuError> {
        let rows = address(relation.row_count())?;
        let count = address(queries.len())?;
        if queries.len() > limits.max_queries || epoch == 0 {
            return Err(capacity(
                "relation query count or invocation epoch exceeds limits",
            ));
        }
        let words = rows.div_ceil(BITS_PER_WORD);
        let empty = rows == 0 || count == 0;
        let groups = if empty {
            [0; 3]
        } else {
            [rows.div_ceil(ROWS_PER_GROUP), count, 1]
        };
        workgroups(groups, device.max_compute_workgroups_per_dimension)?;
        let columns = address(relation.predicate().arity())?;
        let column_bytes = column_bytes(relation, device)?;
        if empty {
            if column_bytes > limits.max_bytes {
                return Err(capacity("relation retained columns exceed byte ceiling"));
            }
            return Ok(Self {
                rows,
                queries: count,
                words,
                workgroups: groups,
                column_bytes,
                query_bytes: 0,
                equality_bytes: 0,
                result_bytes: 0,
                mask_bytes: 0,
                transport_bytes: 0,
                accounted_bytes: column_bytes,
                work: 0,
                columns,
                epoch,
            });
        }
        let equalities = queries.iter().try_fold(0u32, |count, query| {
            count
                .checked_add(address(query.equalities().len())?)
                .ok_or_else(|| capacity("relation equality count exceeds u32"))
        })?;
        let query_bytes = u64::from(cells(count, 4)?.max(4)) * 4;
        let equality_bytes = u64::from(cells(equalities, 2)?.max(2)) * 4;
        let stride = words
            .checked_add(RECEIPT_WORDS)
            .ok_or_else(|| capacity("relation result stride exceeds u32"))?;
        let result_bytes = u64::from(cells(count, stride)?.max(1)) * 4;
        let mask_bytes = u64::from(cells(count, words)?) * 4;
        for bytes in [query_bytes, equality_bytes, result_bytes] {
            buffer(bytes, device)?;
            host_words(bytes)?;
        }
        if PARAM_BYTES > device.max_uniform_buffer_binding_size
            || PARAM_BYTES > device.max_buffer_size
        {
            return Err(capacity("relation uniform exceeds granted device limits"));
        }
        let transport_bytes = PARAM_BYTES + query_bytes + equality_bytes + 2 * result_bytes;
        let accounted_bytes = column_bytes
            + transport_bytes
            + PARAM_BYTES
            + query_bytes
            + equality_bytes
            + mask_bytes;
        if accounted_bytes > limits.max_bytes {
            return Err(capacity(
                "relation invocation exceeds authored byte ceiling",
            ));
        }
        let mut plan = Self {
            rows,
            queries: count,
            words,
            workgroups: groups,
            column_bytes,
            query_bytes,
            equality_bytes,
            result_bytes,
            mask_bytes,
            transport_bytes,
            accounted_bytes,
            work: 0,
            columns,
            epoch,
        };
        for query in queries {
            plan.work = plan
                .work
                .checked_add(u64::from(plan.query_work(query)?))
                .ok_or_else(|| capacity("relation work sum exceeds u64"))?;
        }
        if plan.work > limits.max_work {
            return Err(capacity("relation full-scan work exceeds ceiling"));
        }
        Ok(plan)
    }

    pub(super) fn params(&self) -> [u32; 8] {
        [
            self.rows,
            self.columns,
            self.queries,
            self.words,
            self.epoch,
            0,
            0,
            0,
        ]
    }

    fn query_work(&self, query: &Query<'_, '_>) -> Result<u32, GpuError> {
        if self.rows == 0 {
            return Ok(0);
        }
        let work = u64::from(self.workgroups[0]) * u64::from(ROWS_PER_GROUP)
            + u64::from(self.words) * u64::from(BITS_PER_WORD)
            + 1
            + u64::from(self.rows)
                * u64::try_from(query.equalities().len())
                    .map_err(|_| capacity("relation equality count exceeds u64"))?;
        u32::try_from(work).map_err(|_| capacity("relation query work exceeds u32 receipt"))
    }

    pub(super) fn pack(
        &self,
        queries: &[Query<'_, '_>],
        control: &Control,
    ) -> Result<Packed, GpuError> {
        let query_words = host_words(self.query_bytes)?;
        let equality_words = host_words(self.equality_bytes)?;
        let mut records = vector(query_words)?;
        let mut equalities = vector(equality_words)?;
        for query in queries {
            poll(control)?;
            records.extend_from_slice(&[
                address(equalities.len() / 2)?,
                address(query.equalities().len())?,
                u32::from(query.is_possible()),
                self.query_work(query)?,
            ]);
            for (index, equality) in query.equalities().iter().enumerate() {
                if index % CONTROL_INTERVAL == 0 {
                    poll(control)?;
                }
                equalities.extend_from_slice(&[address(equality.column())?, equality.value_id()]);
            }
        }
        records.resize(query_words, 0);
        equalities.resize(equality_words, 0);
        Ok(Packed {
            records,
            equalities,
        })
    }

    pub(super) fn decode<'owner, 'source>(
        &self,
        relation: &'owner Relation<'source>,
        queries: &[Query<'_, '_>],
        input: &[u32],
        control: &Control,
    ) -> Result<RelationGpuMasks<'owner, 'source>, GpuError> {
        if queries.len() != self.queries as usize || input.len() != host_words(self.result_bytes)? {
            return Err(readback(
                "relation result length differs from complete query population",
            ));
        }
        let stride = (RECEIPT_WORDS + self.words) as usize;
        let mut words = vector(host_words(self.mask_bytes)?)?;
        for (index, (record, query)) in input.chunks_exact(stride).zip(queries).enumerate() {
            poll(control)?;
            if record[..RECEIPT_WORDS as usize]
                != [
                    RECEIPT_MARKER,
                    self.epoch,
                    address(index)?,
                    self.query_work(query)?,
                ]
            {
                return Err(readback("relation query receipt identity or work differs"));
            }
            let mask = &record[RECEIPT_WORDS as usize..];
            let tail = self.rows % BITS_PER_WORD;
            if tail != 0 && mask.last().is_some_and(|word| word >> tail != 0) {
                return Err(readback("relation mask has nonzero unused tail bits"));
            }
            words.extend_from_slice(mask);
        }
        Ok(RelationGpuMasks {
            relation,
            queries: self.queries as usize,
            words_per_query: self.words as usize,
            words,
        })
    }
}

pub(super) struct Packed {
    pub(super) records: Vec<u32>,
    pub(super) equalities: Vec<u32>,
}

pub(super) fn workgroups(groups: [u32; 3], limit: u32) -> Result<u64, GpuError> {
    if groups.into_iter().any(|axis| axis > limit) {
        return Err(capacity(
            "relation dispatch axis exceeds granted device ceiling",
        ));
    }
    groups.into_iter().try_fold(1u64, |product, axis| {
        product
            .checked_mul(u64::from(axis))
            .ok_or_else(|| capacity("relation workgroup product exceeds u64"))
    })
}

fn readback(detail: &str) -> GpuError {
    GpuError::new(GpuErrorKind::Readback, detail)
}
