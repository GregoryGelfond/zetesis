//! Canonical lowering and one request-owned native solve.

use std::{sync::Arc, time::Instant};

use themelios_program::program::{Program, Statement};
use themelios_solve::{
    agent::Scenario,
    bridge::Door,
    contract::{Backend, Cancel, Capabilities, Fault, Presupposition, SolveRequest},
    outcome::{ShowRule, Solved},
};
use zetesis_cpu::{CancellationSlot, CancellationSlotError};
use zetesis_solve::Session;
use zetesis_themelios::validate_program_formula;

use crate::{Config, faults, prepared::Prepared, run::Run};

/// A reusable CPU backend over canonical ASP programs.
///
/// Each successful `lower` replaces the previous program; a refusal preserves it.
/// Lowering only validates bounded input structure and retains its canonical
/// value and display policy. Each `solve` owns fresh preparation and search work,
/// under a single request deadline including consumer pauses.
///
/// A live handle borrows this solver, so its program cannot be replaced during
/// enumeration. Dropping that handle closes its cancellation window. Detached
/// answer sets and snapshots remain independent values. Named/formal program
/// parts are refused at lowering; agent-loop updates use ordinary replacements.
///
/// Models carry the complete answer set, including hidden atoms. Objectives and
/// `#project` are excluded from solve preparation; `#show` affects display only.
/// Native GPU and scored sessions remain available through `zetesis_solve`.
pub struct Solver {
    config: Config,
    cancellation: CancellationSlot,
    lowered: Option<Lowered>,
    prepared: Option<Prepared>,
}

struct Lowered {
    program: Arc<Program>,
    show: ShowRule,
}

impl Solver {
    /// Create an empty backend with explicit bounded preparation and execution
    /// policy. This allocates a cancellation slot but starts no workers, timer,
    /// grounding or search. Configuration ceilings are enforced where used.
    #[must_use]
    pub fn new(config: Config) -> Self {
        Self {
            config,
            cancellation: CancellationSlot::default(),
            lowered: None,
            prepared: None,
        }
    }
}

impl Default for Solver {
    fn default() -> Self {
        Self::new(Config::default())
    }
}

impl Drop for Solver {
    fn drop(&mut self) {
        self.cancellation.clear();
    }
}

impl Backend for Solver {
    fn capabilities(&self) -> Capabilities {
        let mut capabilities = Capabilities::default();
        capabilities.enumeration = true;
        capabilities.cancellation = true;
        capabilities.budgets.time = true;
        capabilities
    }

    fn lower(&mut self, door: Door<'_>) -> Result<(), Fault> {
        let program = door.program();
        validate_program_formula(program, self.config.resources().program_admission_options())
            .map_err(faults::admission)?;
        let lowered = Lowered {
            program: Arc::new(program.clone()),
            show: ShowRule::of(program.statements().filter_map(
                |statement| match statement.get() {
                    Statement::Show(show) => Some(show),
                    _ => None,
                },
            )),
        };
        self.lowered = Some(lowered);
        self.prepared = None;
        Ok(())
    }

    fn solve(&mut self, request: &SolveRequest) -> Result<Solved<'_>, Fault> {
        let started = Instant::now();
        let deadline = request
            .time
            .map(|duration| {
                started.checked_add(duration).ok_or_else(|| {
                    Fault::request(
                        "the requested deadline cannot be represented on this platform",
                        Presupposition::UnrealisableBudget,
                    )
                })
            })
            .transpose()?;
        // Activate before releasing old preparation, copying display policy,
        // compiling a program, or allocating native workers. A pull during
        // setup therefore reaches this request, including its preparation.
        let window = self.cancellation.open(deadline).map_err(window_fault)?;
        self.prepared = None;
        let lowered = self.lowered.as_ref().ok_or_else(|| {
            Fault::request(
                "lower a program before solving",
                Presupposition::NeedsRebuild,
            )
        })?;
        let show = lowered.show.clone();
        let prepared = Prepared::new(
            Arc::clone(&lowered.program),
            &self.config,
            window.cancellation(),
        );
        let prepared = match prepared {
            Ok(prepared) => prepared,
            Err(error) => {
                let conclusion = faults::formula(error, &lowered.program)?;
                return Ok(Solved::running(
                    Box::new(Run::pending(conclusion, window)),
                    Scenario::default(),
                    show,
                ));
            }
        };
        self.prepared = Some(prepared);
        let owner = self
            .prepared
            .as_ref()
            .expect("successful preparation was retained");
        let session = Session::enumerate(
            owner.input(),
            self.config.session(),
            window.cancellation().clone(),
        );
        let run = match session {
            Ok(session) => Run::new(
                session,
                &lowered.program,
                owner.metadata(),
                window,
                self.config.output(),
            ),
            Err(error) => Run::pending(faults::session(error, &lowered.program)?, window),
        };
        Ok(Solved::running(Box::new(run), Scenario::default(), show))
    }

    fn interrupt(&self) -> Option<Box<dyn Cancel>> {
        Some(Box::new(Interrupt(self.cancellation.clone())))
    }
}

struct Interrupt(CancellationSlot);

impl Cancel for Interrupt {
    fn cancel(&self) {
        self.0.cancel();
    }
}

fn window_fault(error: CancellationSlotError) -> Fault {
    Fault::resource(error.to_string()).caused_by(error)
}
