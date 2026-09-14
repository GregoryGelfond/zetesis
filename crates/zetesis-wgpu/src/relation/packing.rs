use super::{
    BITS_PER_WORD, PARAM_BYTES, RECEIPT_MARKER, RECEIPT_WORDS, ROWS_PER_GROUP, RelationGpuLimits,
    RelationGpuMasks, RelationGpuStats, capacity, poll,
};
use crate::{GpuError, GpuErrorKind};
use zetesis_core::relation::{Query, Relation};
use zetesis_cpu::Control;

const CONTROL_INTERVAL: usize = 1024;

pub(super) fn vector(length: usize) -> Result<Vec<u32>, GpuError> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(length)
        .map_err(|_| GpuError::new(GpuErrorKind::Allocation, "relation host reservation failed"))?;
    Ok(values)
}

fn retained_bytes(values: &Vec<u32>) -> Result<u64, GpuError> {
    u64::try_from(values.capacity())
        .ok()
        .and_then(|count| count.checked_mul(4))
        .ok_or_else(|| capacity("relation retained host capacity exceeds u64"))
}

pub(super) fn accounted_bytes(
    columns: u64,
    transport: u64,
    host: [u64; 3],
    limit: u64,
) -> Result<u64, GpuError> {
    let bytes = [columns, transport, PARAM_BYTES, host[0], host[1], host[2]]
        .into_iter()
        .try_fold(0u64, u64::checked_add)
        .ok_or_else(|| capacity("relation retained payload sum exceeds u64"))?;
    if bytes > limit {
        return Err(capacity(
            "relation invocation exceeds authored byte ceiling",
        ));
    }
    Ok(bytes)
}

fn address(value: usize) -> Result<u32, GpuError> {
    u32::try_from(value).map_err(|_| capacity("relation dimension exceeds u32"))
}

fn cells(left: u32, right: u32) -> Result<u32, GpuError> {
    left.checked_mul(right)
        .ok_or_else(|| capacity("relation cell offset exceeds u32"))
}

// Each query owns all tile receipts followed by its packed mask words. Admit
// the full population as well as its per-query stride before any allocation.
fn result_layout(words: u32, tiles: u32, queries: u32) -> Result<(u32, u64), GpuError> {
    let stride = cells(tiles, RECEIPT_WORDS)?
        .checked_add(words)
        .ok_or_else(|| capacity("relation result stride exceeds u32"))?;
    Ok((stride, u64::from(cells(queries, stride)?) * 4))
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
    if relation.columns().len() != relation.predicate().arity()
        || relation
            .columns()
            .any(|column| column.len() != relation.row_count())
    {
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
    result_stride: u32,
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
                result_stride: 0,
            });
        }
        let equalities = queries.iter().try_fold(0u32, |count, query| {
            count
                .checked_add(address(query.equalities().len())?)
                .ok_or_else(|| capacity("relation equality count exceeds u32"))
        })?;
        crate::runtime::positive_timeout(limits.timeout)?;
        let query_bytes = u64::from(cells(count, 4)?.max(4)) * 4;
        let equality_bytes = u64::from(cells(equalities, 2)?.max(2)) * 4;
        let (result_stride, result_bytes) = result_layout(words, groups[0], count)?;
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
        let accounted_bytes = accounted_bytes(
            column_bytes,
            transport_bytes,
            [query_bytes, equality_bytes, mask_bytes],
            limits.max_bytes,
        )?;
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
            result_stride,
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
            self.workgroups[0],
            0,
            0,
        ]
    }

    pub(super) fn stats(&self) -> RelationGpuStats {
        RelationGpuStats {
            rows: u64::from(self.rows),
            queries: u64::from(self.queries),
            column_bytes: self.column_bytes,
            transport_bytes: self.transport_bytes,
            accounted_bytes: self.accounted_bytes,
            workgroups: self.workgroups,
        }
    }

    fn query_work(&self, query: &Query<'_, '_>) -> Result<u32, GpuError> {
        if self.rows == 0 {
            return Ok(0);
        }
        let work = u64::from(self.workgroups[0]) * u64::from(ROWS_PER_GROUP)
            + u64::from(self.words) * u64::from(BITS_PER_WORD)
            + u64::from(self.workgroups[0])
            + u64::from(self.rows)
                * u64::try_from(query.equalities().len())
                    .map_err(|_| capacity("relation equality count exceeds u64"))?;
        u32::try_from(work).map_err(|_| capacity("relation query work exceeds u32 receipt"))
    }

    pub(super) fn pack(
        &mut self,
        queries: &[Query<'_, '_>],
        control: &Control,
        max_bytes: u64,
    ) -> Result<Packed, GpuError> {
        self.pack_with(queries, control, max_bytes, vector)
    }

    // The reservation operation returns an empty vector with at least the
    // requested capacity. The same path admits ordinary and test reservations.
    pub(super) fn pack_with(
        &mut self,
        queries: &[Query<'_, '_>],
        control: &Control,
        max_bytes: u64,
        mut reserve: impl FnMut(usize) -> Result<Vec<u32>, GpuError>,
    ) -> Result<Packed, GpuError> {
        if queries.len() != self.queries as usize {
            return Err(capacity("relation packing query population differs"));
        }
        let minimum = [self.query_bytes, self.equality_bytes, self.mask_bytes];
        let mut host = minimum;
        let mut allocate = |index| {
            poll(control)?;
            let length = host_words(minimum[index])?;
            let mut values = reserve(length)?;
            if !values.is_empty() || values.capacity() < length {
                return Err(capacity("relation reservation cannot hold planned output"));
            }
            host[index] = retained_bytes(&values)?;
            accounted_bytes(self.column_bytes, self.transport_bytes, host, max_bytes)?;
            poll(control)?;
            values.resize(length, 0);
            Ok(values)
        };
        let mut records = allocate(0)?;
        let mut equalities = allocate(1)?;
        let masks = allocate(2)?;
        self.accounted_bytes =
            accounted_bytes(self.column_bytes, self.transport_bytes, host, max_bytes)?;
        let mut offset = 0;
        for (record, query) in records.chunks_exact_mut(4).zip(queries) {
            poll(control)?;
            record.copy_from_slice(&[
                address(offset / 2)?,
                address(query.equalities().len())?,
                u32::from(query.is_possible()),
                self.query_work(query)?,
            ]);
            let end = query
                .equalities()
                .len()
                .checked_mul(2)
                .and_then(|count| offset.checked_add(count))
                .ok_or_else(|| capacity("relation packing equality offset exceeds usize"))?;
            let targets = equalities
                .get_mut(offset..end)
                .ok_or_else(|| capacity("relation packing equality population differs"))?;
            for (index, (target, equality)) in targets
                .chunks_exact_mut(2)
                .zip(query.equalities())
                .enumerate()
            {
                if index % CONTROL_INTERVAL == 0 {
                    poll(control)?;
                }
                target.copy_from_slice(&[address(equality.column())?, equality.value_id()]);
            }
            offset = end;
        }
        Ok(Packed {
            records,
            equalities,
            masks,
        })
    }

    pub(super) fn decode<'owner, 'source>(
        &self,
        relation: &'owner Relation<'source>,
        queries: &[Query<'_, '_>],
        input: &[u32],
        mut words: Vec<u32>,
        control: &Control,
    ) -> Result<RelationGpuMasks<'owner, 'source>, GpuError> {
        if queries.len() != self.queries as usize
            || input.len() != host_words(self.result_bytes)?
            || words.len() != host_words(self.mask_bytes)?
        {
            return Err(readback(
                "relation result length differs from complete query population",
            ));
        }
        if self.result_bytes != 0 {
            self.decode_records(queries, input, &mut words, control)?;
        }
        Ok(RelationGpuMasks {
            relation,
            queries: self.queries as usize,
            words_per_query: self.words as usize,
            words,
        })
    }

    fn decode_records(
        &self,
        queries: &[Query<'_, '_>],
        input: &[u32],
        words: &mut [u32],
        control: &Control,
    ) -> Result<(), GpuError> {
        let stride = self.result_stride as usize;
        // The checked layout established this prefix and the complete input
        // length above prevents a short final query from disappearing in zip.
        let receipts = stride - self.words as usize;
        for (index, (record, query)) in input.chunks_exact(stride).zip(queries).enumerate() {
            let work = self.query_work(query)?;
            for (tile, receipt) in record[..receipts]
                .chunks_exact(RECEIPT_WORDS as usize)
                .enumerate()
            {
                poll(control)?;
                if receipt
                    != [
                        RECEIPT_MARKER,
                        self.epoch,
                        address(index)?,
                        address(tile)?,
                        work,
                    ]
                {
                    return Err(readback("relation tile receipt identity or work differs"));
                }
            }
            let mask = &record[receipts..];
            let tail = self.rows % BITS_PER_WORD;
            if tail != 0 && mask.last().is_some_and(|word| word >> tail != 0) {
                return Err(readback("relation mask has nonzero unused tail bits"));
            }
            let start = index * self.words as usize;
            words[start..start + mask.len()].copy_from_slice(mask);
        }
        Ok(())
    }
}

pub(super) struct Packed {
    pub(super) records: Vec<u32>,
    pub(super) equalities: Vec<u32>,
    pub(super) masks: Vec<u32>,
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

#[cfg(test)]
#[path = "../../tests/relation/layout.rs"]
mod tests;
