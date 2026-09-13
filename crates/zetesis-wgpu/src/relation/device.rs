use super::{
    PARAM_BYTES, ROWS_PER_GROUP, RelationGpuActivity, RelationGpuError, RelationGpuLimits,
    RelationGpuMasks, RelationGpuStats, SHADER, capacity, packing, poll,
};
use crate::{GpuError, GpuErrorKind, GpuInfo, GpuOptions, GpuSelection, runtime};
use zetesis_core::relation::{Failure, Query, Relation};
use zetesis_cpu::Control;

/// Reusable real-device pipeline for checked relation equality filtering.
///
/// A prepared relation exclusively borrows the executor. No global cache,
/// persistent raw-address key or duplicate logical row owner is introduced.
pub struct GpuRelationExecutor {
    runtime: runtime::Runtime,
    epoch: u32,
}

impl GpuRelationExecutor {
    /// Create the selected real adapter, device and equality-mask pipeline.
    ///
    /// # Errors
    /// Reports adapter absence, policy refusal, unsupported device limits,
    /// allocation, device or shader validation failure. There is no CPU fallback.
    pub fn new_selected(
        options: GpuOptions,
        selection: GpuSelection,
    ) -> Result<Self, RelationGpuError> {
        let runtime =
            pollster::block_on(runtime::Runtime::new(options, selection, Self::profile()))?;
        Ok(Self::with_runtime(runtime))
    }

    /// Compile this primitive on an existing device context.
    ///
    /// Prepared subjects belong to this primitive; device health and execution
    /// serialization are shared with all primitives using the context.
    ///
    /// # Errors
    /// Refuses an active or invalidated context, unsupported granted limits,
    /// allocation and shader validation failure. No adapter selection occurs.
    pub fn from_context(context: &crate::GpuContext) -> Result<Self, RelationGpuError> {
        let runtime = pollster::block_on(runtime::Runtime::from_context(context, Self::profile()))?;
        Ok(Self::with_runtime(runtime))
    }

    /// Exact device context retained by this primitive.
    #[must_use]
    pub fn context(&self) -> &crate::GpuContext {
        &self.runtime.context
    }

    fn with_runtime(runtime: runtime::Runtime) -> Self {
        Self { runtime, epoch: 0 }
    }

    fn profile() -> runtime::DeviceProfile {
        runtime::DeviceProfile {
            device_label: "zetesis relation executor",
            shader_label: "zetesis relation equality masks",
            pipeline_label: "zetesis relation row tiles",
            shader: SHADER.into(),
            entry_point: "select_rows",
            validate_limits: adapter_limits,
        }
    }

    /// Identity of the actual selected adapter.
    #[must_use]
    pub fn info(&self) -> &GpuInfo {
        self.runtime.context.info()
    }

    /// Upload a checked immutable column view without copying logical values.
    ///
    /// Dropping the prepared view releases its authored column-buffer handle and
    /// makes this executor available for another relation. Other primitives sharing
    /// the context may execute while this prepared view remains live; preparation
    /// does not retain the context's execution lease. Driver retirement can
    /// occur later. Source dictionary, query resolution and typed rows stay with
    /// the borrowed Relation. The preparation limit charges uploaded columns;
    /// filtering separately charges its complete transport and returned masks.
    ///
    /// # Errors
    /// Refuses cancellation, invalidated device health, exceeded host/device
    /// capacities, allocation and upload errors before returning a prepared view.
    /// Cancellation during mapped host copying unmaps and releases the incomplete
    /// buffer before scope settlement. If those scopes and device health succeed,
    /// the context remains reusable: preparation submits no queue work.
    pub fn prepare<'device, 'owner, 'source>(
        &'device mut self,
        relation: &'owner Relation<'source>,
        limits: RelationGpuLimits,
        control: &Control,
    ) -> Result<PreparedGpuRelation<'device, 'owner, 'source>, RelationGpuError> {
        self.prepare_with(relation, limits, || poll(control))
    }

    fn prepare_with<'device, 'owner, 'source>(
        &'device mut self,
        relation: &'owner Relation<'source>,
        limits: RelationGpuLimits,
        mut control: impl FnMut() -> Result<(), GpuError>,
    ) -> Result<PreparedGpuRelation<'device, 'owner, 'source>, RelationGpuError> {
        let context = self.runtime.context.clone();
        let _lease = context.lease()?;
        control()?;
        self.runtime.check_health()?;
        let bytes = packing::column_bytes(relation, self.runtime.limits())?;
        if bytes > limits.max_bytes {
            return Err(capacity("relation upload exceeds authored byte ceiling").into());
        }
        let scopes = runtime::ErrorScopes::new(self.runtime.device());
        let columns = upload_columns(self.runtime.device(), relation, bytes, &mut control);
        let columns = self.runtime.complete_unsubmitted(scopes, columns)?;
        Ok(PreparedGpuRelation {
            executor: self,
            relation,
            columns,
            column_bytes: bytes,
            activity: RelationGpuActivity::default(),
            last: None,
        })
    }
}

/// Uploaded columns borrowed from one exact relation and one device executor.
pub struct PreparedGpuRelation<'device, 'owner, 'source> {
    executor: &'device mut GpuRelationExecutor,
    relation: &'owner Relation<'source>,
    columns: wgpu::Buffer,
    column_bytes: u64,
    activity: RelationGpuActivity,
    last: Option<RelationGpuStats>,
}

impl<'owner, 'source> PreparedGpuRelation<'_, 'owner, 'source> {
    /// Identity of the real adapter owning this prepared column buffer.
    #[must_use]
    pub fn info(&self) -> &GpuInfo {
        self.executor.info()
    }

    /// Original immutable relation, including typed rows and catalog mapping.
    #[must_use]
    pub const fn relation(&self) -> &'owner Relation<'source> {
        self.relation
    }

    /// Bytes uploaded by preparation, including padding for zero cells.
    #[must_use]
    pub const fn column_bytes(&self) -> u64 {
        self.column_bytes
    }

    /// Current attempt's actual submission and completion evidence.
    #[must_use]
    pub const fn activity(&self) -> RelationGpuActivity {
        self.activity
    }

    /// Resource observations for the last complete filtering call.
    #[must_use]
    pub const fn last_stats(&self) -> Option<RelationGpuStats> {
        self.last
    }

    /// Filter query occurrences into original-row masks on the actual device.
    ///
    /// Every query must borrow this exact Relation. Query order and repetitions
    /// are preserved; missing dictionary values produce empty masks and empty
    /// equality conjunctions select every row. Zero rows or zero queries complete
    /// without submission, while still observing control and device health.
    /// A full host pattern match remains necessary after equality filtering.
    ///
    /// Each nonempty call allocates exact batch transport, uses the already
    /// uploaded columns, and releases transport handles after readback. A later
    /// experiment may justify transport reuse; no such cache is hidden here.
    /// Host packing and the complete output-mask vector are reserved before
    /// device scopes open. Their actual retained capacities, including spare
    /// elements, must fit the total byte ceiling. Readback fills that output
    /// storage without further reservation or vector growth.
    ///
    /// # Errors
    /// Refuses foreign query owners, capacity, allocation, cancellation and device
    /// or readback failure. No partial mask batch is returned. Post-submit errors
    /// invalidate the entire shared context; pre-dispatch ownership/capacity and
    /// Busy refusals leave it reusable. Error-scope and device faults take priority.
    pub fn filter(
        &mut self,
        queries: &[Query<'_, 'source>],
        limits: RelationGpuLimits,
        control: &Control,
    ) -> Result<RelationGpuMasks<'owner, 'source>, RelationGpuError> {
        self.activity = RelationGpuActivity::default();
        self.last = None;
        let context = self.executor.runtime.context.clone();
        let _lease = context.lease()?;
        poll(control)?;
        self.executor.runtime.check_health()?;
        if queries.len() > limits.max_queries || u32::try_from(queries.len()).is_err() {
            return Err(capacity("relation query count exceeds preflight ceiling").into());
        }
        if queries
            .iter()
            .any(|query| !self.relation.same_owner(query.relation()))
        {
            return Err(RelationGpuError::Relation(Failure::Owner));
        }
        let epoch = self
            .executor
            .epoch
            .checked_add(1)
            .ok_or_else(|| capacity("relation invocation epoch exhausted"))?;
        let mut plan = packing::Plan::new(
            self.relation,
            queries,
            limits,
            self.executor.runtime.limits(),
            epoch,
        )?;
        if plan.rows == 0 || plan.queries == 0 {
            self.last = Some(plan.stats());
            return Ok(RelationGpuMasks {
                relation: self.relation,
                queries: queries.len(),
                words_per_query: plan.words as usize,
                words: Vec::new(),
            });
        }
        let packed = plan.pack(queries, control, limits.max_bytes)?;
        poll(control)?;
        self.executor.epoch = epoch;
        let runtime = &mut self.executor.runtime;
        let scopes = runtime::ErrorScopes::new(runtime.device());
        let transport = Transport::new(runtime, &self.columns, &plan, &packed);
        self.activity.uploaded_bytes = PARAM_BYTES + plan.query_bytes + plan.equality_bytes;
        let outcome = poll(control).map(|()| {
            let submission = runtime::submit(
                runtime.device(),
                runtime.queue(),
                &runtime::Dispatch {
                    command_label: "zetesis relation commands",
                    pass_label: "zetesis relation row tiles",
                    pipeline: &runtime.pipeline,
                    group: &transport.group,
                    workgroups: plan.workgroups,
                    result: &transport.result,
                    readback: &transport.readback,
                    result_bytes: plan.result_bytes,
                },
            );
            self.activity.submissions = 1;
            self.activity.submitted_queries = u64::from(plan.queries);
            self.activity.submitted_workgroups =
                u64::from(plan.workgroups[0]) * u64::from(plan.workgroups[1]);
            self.activity.scheduled_work = plan.work;
            runtime::read_polled(
                runtime.device(),
                &transport.readback,
                submission,
                limits.timeout,
                || poll(control),
                |words| plan.decode(self.relation, queries, words, packed.masks, control),
            )
        });
        let masks = runtime.complete(scopes, outcome)?;
        self.activity.completed_queries = u64::from(plan.queries);
        self.activity.completed_work = plan.work;
        self.activity.downloaded_bytes = plan.result_bytes;
        self.last = Some(plan.stats());
        Ok(masks)
    }
}

struct Transport {
    group: wgpu::BindGroup,
    result: wgpu::Buffer,
    readback: wgpu::Buffer,
}

impl Transport {
    fn new(
        runtime: &runtime::Runtime,
        columns: &wgpu::Buffer,
        plan: &packing::Plan,
        packed: &packing::Packed,
    ) -> Self {
        let device = runtime.device();
        let params = runtime::initialized(
            device,
            "zetesis relation dimensions",
            &plan.params(),
            wgpu::BufferUsages::UNIFORM,
        );
        let queries = runtime::initialized(
            device,
            "zetesis relation queries",
            &packed.records,
            wgpu::BufferUsages::STORAGE,
        );
        let equalities = runtime::initialized(
            device,
            "zetesis relation equalities",
            &packed.equalities,
            wgpu::BufferUsages::STORAGE,
        );
        let result = runtime::buffer(
            device,
            "zetesis relation masks",
            plan.result_bytes,
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        );
        let readback = runtime::buffer(
            device,
            "zetesis relation readback",
            plan.result_bytes,
            wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        );
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("zetesis relation bindings"),
            layout: &runtime.pipeline.get_bind_group_layout(0),
            entries: &[
                runtime::entry(0, &params),
                runtime::entry(1, columns),
                runtime::entry(2, &queries),
                runtime::entry(3, &equalities),
                runtime::entry(4, &result),
            ],
        });
        Self {
            group,
            result,
            readback,
        }
    }
}

fn adapter_limits(limits: &wgpu::Limits) -> Result<(), GpuError> {
    for (name, required, available) in [
        (
            "workgroup invocations",
            ROWS_PER_GROUP,
            limits.max_compute_invocations_per_workgroup,
        ),
        (
            "workgroup width",
            ROWS_PER_GROUP,
            limits.max_compute_workgroup_size_x,
        ),
        (
            "shared row flags",
            ROWS_PER_GROUP * 4,
            limits.max_compute_workgroup_storage_size,
        ),
        (
            "storage bindings",
            4,
            limits.max_storage_buffers_per_shader_stage,
        ),
        (
            "uniform bindings",
            1,
            limits.max_uniform_buffers_per_shader_stage,
        ),
        ("bindings per group", 5, limits.max_bindings_per_bind_group),
    ] {
        if available < required {
            return Err(capacity(&format!(
                "relation {name}: need {required}, device provides {available}"
            )));
        }
    }
    Ok(())
}

/// Copy ordered columns directly into the mapped upload buffer. No temporary
/// packed host vector or duplicated logical tuple owner is retained.
fn upload_columns(
    device: &wgpu::Device,
    relation: &Relation<'_>,
    bytes: u64,
    mut control: impl FnMut() -> Result<(), GpuError>,
) -> Result<wgpu::Buffer, GpuError> {
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("zetesis immutable relation columns"),
        size: bytes,
        usage: wgpu::BufferUsages::STORAGE,
        mapped_at_creation: true,
    });
    let copied = (|| {
        let mut mapped = buffer
            .slice(..)
            .get_mapped_range_mut()
            .map_err(|error| GpuError::new(GpuErrorKind::Validation, error.to_string()))?;
        if relation.row_count() == 0 || relation.predicate().arity() == 0 {
            mapped.copy_from_slice(bytemuck::bytes_of(&0_u32));
        } else {
            let mut offset = 0_usize;
            for source in relation.columns() {
                control()?;
                let source = bytemuck::cast_slice(source);
                let end = offset
                    .checked_add(source.len())
                    .filter(|&end| end <= mapped.len())
                    .ok_or_else(|| capacity("relation column exceeds mapped upload"))?;
                mapped.slice(offset..end).copy_from_slice(source);
                offset = end;
            }
            if offset != mapped.len() {
                return Err(capacity("relation columns do not fill mapped upload"));
            }
        }
        control()
    })();
    buffer.unmap();
    copied?;
    Ok(buffer)
}

#[cfg(test)]
#[path = "../../tests/relation/preparation.rs"]
mod preparation_tests;
