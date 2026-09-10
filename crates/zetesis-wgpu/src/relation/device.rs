use super::{
    PARAM_BYTES, ROWS_PER_GROUP, RelationGpuActivity, RelationGpuError, RelationGpuLimits,
    RelationGpuMasks, RelationGpuStats, SHADER, capacity, packing, poll,
};
use crate::{GpuError, GpuInfo, GpuOptions, GpuSelection, runtime};
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
        let runtime = pollster::block_on(runtime::Runtime::new(
            options,
            selection,
            runtime::DeviceProfile {
                device_label: "zetesis relation executor",
                shader_label: "zetesis relation equality masks",
                pipeline_label: "zetesis relation row tiles",
                shader: SHADER.into(),
                entry_point: "select_rows",
                validate_limits: adapter_limits,
            },
        ))?;
        Ok(Self { runtime, epoch: 0 })
    }

    /// Identity of the actual selected adapter.
    #[must_use]
    pub fn info(&self) -> &GpuInfo {
        &self.runtime.info
    }

    /// Upload a checked immutable column view without copying logical values.
    ///
    /// Dropping the prepared view releases its authored column-buffer handle and
    /// makes this executor available for another relation. Driver retirement can
    /// occur later. Source dictionary, query resolution and typed rows stay with
    /// the borrowed Relation. The preparation limit charges uploaded columns;
    /// filtering separately charges its complete transport and returned masks.
    ///
    /// # Errors
    /// Refuses cancellation, invalidated device health, exceeded host/device
    /// capacities, allocation and upload errors before returning a prepared view.
    pub fn prepare<'device, 'owner, 'source>(
        &'device mut self,
        relation: &'owner Relation<'source>,
        limits: RelationGpuLimits,
        control: &Control,
    ) -> Result<PreparedGpuRelation<'device, 'owner, 'source>, RelationGpuError> {
        poll(control)?;
        self.runtime.check_health()?;
        let bytes = packing::column_bytes(relation, &self.runtime.limits)?;
        if bytes > limits.max_bytes {
            return Err(capacity("relation upload exceeds authored byte ceiling").into());
        }
        let scopes = runtime::ErrorScopes::new(&self.runtime.device);
        let cells = if relation.columns().is_empty() {
            &[0]
        } else {
            relation.columns()
        };
        let columns = runtime::initialized(
            &self.runtime.device,
            "zetesis immutable relation columns",
            cells,
            wgpu::BufferUsages::STORAGE,
        );
        self.runtime.complete(scopes, poll(control))?;
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
    ///
    /// # Errors
    /// Refuses foreign query owners, capacity, allocation, cancellation and device
    /// or readback failure. No partial mask batch is returned. Post-submit errors
    /// invalidate the executor; pre-dispatch ownership/capacity refusals leave it
    /// reusable. Runtime error-scope and asynchronous device faults take priority.
    pub fn filter(
        &mut self,
        queries: &[Query<'_, 'source>],
        limits: RelationGpuLimits,
        control: &Control,
    ) -> Result<RelationGpuMasks<'owner, 'source>, RelationGpuError> {
        self.activity = RelationGpuActivity::default();
        self.last = None;
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
        let plan = packing::Plan::new(
            self.relation,
            queries,
            limits,
            &self.executor.runtime.limits,
            epoch,
        )?;
        let stats = RelationGpuStats {
            rows: u64::from(plan.rows),
            queries: u64::from(plan.queries),
            column_bytes: plan.column_bytes,
            transport_bytes: plan.transport_bytes,
            accounted_bytes: plan.accounted_bytes,
            workgroups: plan.workgroups,
        };
        if plan.rows == 0 || plan.queries == 0 {
            self.last = Some(stats);
            return Ok(RelationGpuMasks {
                relation: self.relation,
                queries: queries.len(),
                words_per_query: plan.words as usize,
                words: Vec::new(),
            });
        }
        let packed = plan.pack(queries, control)?;
        poll(control)?;
        self.executor.epoch = epoch;
        let runtime = &mut self.executor.runtime;
        let scopes = runtime::ErrorScopes::new(&runtime.device);
        let transport = Transport::new(runtime, &self.columns, &plan, &packed);
        self.activity.uploaded_bytes = PARAM_BYTES + plan.query_bytes + plan.equality_bytes;
        let outcome = poll(control).and_then(|()| {
            let submission = runtime::submit(
                &runtime.device,
                &runtime.queue,
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
                &runtime.device,
                &transport.readback,
                submission,
                limits.timeout,
                || poll(control),
                |words| plan.decode(self.relation, queries, words, control),
            )
        });
        let masks = runtime.complete(scopes, outcome)?;
        self.activity.completed_queries = u64::from(plan.queries);
        self.activity.completed_work = plan.work;
        self.activity.downloaded_bytes = plan.result_bytes;
        self.last = Some(stats);
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
        let device = &runtime.device;
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
