//! Exact, small, finite semantics. No device, parser, learning, or network code.
pub mod lifted;
use std::collections::BTreeSet;

/// Dense finite set of atoms, with atom `a` represented by bit `a`.
pub type Mask = u64;
/// Reference operation outcome, with a descriptive admission or invariant failure.
pub type Result<T> = std::result::Result<T, String>;

/// A source-level rule in the supported fragment. Duplicate antecedents are
/// canonicalized by the bit mask. `choice` means a SINGLETON choice head.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Rule {
    /// Head atom, or absence for an integrity constraint.
    pub head: Option<usize>,
    /// Whether the head contributes its own positive reduct gate.
    pub choice: bool,
    /// Ordinary positive body atoms.
    pub positive: Mask,
    /// Body atoms under default negation.
    pub negative: Mask,
    /// Body atoms under double default negation.
    pub double_negative: Mask,
}

impl Rule {
    /// Construct an ordinary rule; carrier validation occurs in [`Program::new`].
    #[must_use]
    pub fn normal(head: usize, positive: Mask, negative: Mask, double_negative: Mask) -> Self {
        Self {
            head: Some(head),
            choice: false,
            positive,
            negative,
            double_negative,
        }
    }
    /// Construct a singleton choice whose head is tested against the frozen seed.
    #[must_use]
    pub fn singleton_choice(head: usize, positive: Mask, negative: Mask) -> Self {
        Self {
            head: Some(head),
            choice: true,
            positive,
            negative,
            double_negative: 0,
        }
    }
    /// Construct a headless constraint; carrier validation occurs on admission.
    #[must_use]
    pub fn constraint(positive: Mask, negative: Mask, double_negative: Mask) -> Self {
        Self {
            head: None,
            choice: false,
            positive,
            negative,
            double_negative,
        }
    }
}

/// Validated finite ground rules over fewer than 64 densely numbered atoms.
#[derive(Clone, Debug)]
pub struct Program {
    atoms: usize,
    universe: Mask,
    seed_carrier: Mask,
    rules: Vec<Rule>,
}

/// Exact reduct-closure result for one total assignment on the seed carrier.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeedCheck {
    /// Least closure of the positive rules selected by the frozen seed.
    pub closure: Mask,
    /// Whether closure agrees with the seed and satisfies every active constraint.
    pub stable: bool,
    /// Whether an active constraint has its complete body in the closure.
    pub constraint_violated: bool,
}

/// Seed assignments between an inclusive lower and upper set bound.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Cube {
    /// Atoms required in every represented seed.
    pub lower: Mask,
    /// Atoms permitted in any represented seed.
    pub upper: Mask,
}

/// One replayable enclosure and narrowing step for a seed cube.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NarrowStep {
    /// Input seed bounds.
    pub before: Cube,
    /// Closure of rules active under every represented seed.
    pub lower_closure: Mask,
    /// Closure of rules active under at least one represented seed.
    pub upper_closure: Mask,
    /// Bounds after intersecting seed agreement with both closures.
    pub after: Cube,
    /// Whether the resulting lower bound is outside the upper bound.
    pub inconsistent: bool,
    /// First constraint violated by the lower closure, when one exists.
    pub definite_constraint: Option<usize>,
}

impl NarrowStep {
    /// Whether this step certifies that the input cube contains no stable seed.
    #[must_use]
    pub fn rejected(&self) -> bool {
        self.inconsistent || self.definite_constraint.is_some()
    }
}

impl Program {
    /// Validate and own a finite rule sequence and its dense atom carrier.
    ///
    /// # Errors
    /// Returns an error for 64 or more atoms, an out-of-carrier head/body atom,
    /// or a singleton choice without a head.
    pub fn new(atoms: usize, rules: Vec<Rule>) -> Result<Self> {
        if atoms >= 64 {
            return Err("atom count must be below 64".into());
        }
        let universe = (1u64 << atoms) - 1;
        for (i, r) in rules.iter().enumerate() {
            if r.head.is_some_and(|h| h >= atoms) {
                return Err(format!("rule {i}: head outside atom carrier"));
            }
            if r.choice && r.head.is_none() {
                return Err(format!("rule {i}: a singleton choice needs a head"));
            }
            if (r.positive | r.negative | r.double_negative) & !universe != 0 {
                return Err(format!("rule {i}: body outside atom carrier"));
            }
        }
        let seed_carrier = rules
            .iter()
            .fold(0, |s, r| s | r.negative | Self::gate_positive(r));
        Ok(Self {
            atoms,
            universe,
            seed_carrier,
            rules,
        })
    }
    /// Number of atoms in the validated dense carrier.
    #[must_use]
    pub fn atom_count(&self) -> usize {
        self.atoms
    }
    /// Atoms read by reduct gates, including gates in constraints and choices.
    #[must_use]
    pub fn seed_carrier(&self) -> Mask {
        self.seed_carrier
    }
    /// Validated rules in their original order, with duplicate rules preserved.
    #[must_use]
    pub fn rules(&self) -> &[Rule] {
        &self.rules
    }
    fn gate_positive(r: &Rule) -> Mask {
        r.double_negative
            | if r.choice {
                1u64 << r.head.expect("validated choice")
            } else {
                0
            }
    }
    fn enabled(r: &Rule, seed: Mask) -> bool {
        Self::gate_positive(r) & !seed == 0 && r.negative & seed == 0
    }
    fn validate_seed(&self, seed: Mask) -> Result<()> {
        if seed & !self.seed_carrier != 0 {
            return Err("seed outside seed carrier".into());
        }
        Ok(())
    }
    fn validate_cube(&self, c: Cube) -> Result<()> {
        if c.lower & !c.upper != 0 || c.upper & !self.seed_carrier != 0 {
            return Err("invalid seed cube".into());
        }
        Ok(())
    }
    fn positive_closure(&self, active: &[bool]) -> Mask {
        let mut derived = 0;
        loop {
            let mut next = derived;
            for (r, &on) in self.rules.iter().zip(active) {
                if on
                    && r.positive & !derived == 0
                    && let Some(h) = r.head
                {
                    next |= 1u64 << h;
                }
            }
            if next == derived {
                return derived;
            }
            derived = next;
        }
    }
    /// Compute reduct closure and exact seed agreement.
    ///
    /// # Errors
    /// Returns an error when the seed contains an atom outside the seed carrier.
    pub fn check_seed(&self, seed: Mask) -> Result<SeedCheck> {
        self.validate_seed(seed)?;
        let active: Vec<_> = self.rules.iter().map(|r| Self::enabled(r, seed)).collect();
        let closure = self.positive_closure(&active);
        let constraint_violated = self
            .rules
            .iter()
            .zip(active)
            .any(|(r, on)| on && r.head.is_none() && r.positive & !closure == 0);
        Ok(SeedCheck {
            closure,
            stable: closure & self.seed_carrier == seed && !constraint_violated,
            constraint_violated,
        })
    }
    /// Enclose every completion's closure and narrow the cube by seed agreement.
    ///
    /// # Errors
    /// Returns an error if the lower set exceeds the upper set or the upper set
    /// contains an atom outside the seed carrier.
    pub fn narrow(&self, c: Cube) -> Result<NarrowStep> {
        self.validate_cube(c)?;
        let must: Vec<_> = self
            .rules
            .iter()
            .map(|r| Self::gate_positive(r) & !c.lower == 0 && r.negative & c.upper == 0)
            .collect();
        let may: Vec<_> = self
            .rules
            .iter()
            .map(|r| {
                let pos = Self::gate_positive(r);
                pos & r.negative == 0 && pos & !c.upper == 0 && r.negative & c.lower == 0
            })
            .collect();
        let lower_closure = self.positive_closure(&must);
        let upper_closure = self.positive_closure(&may);
        let after = Cube {
            lower: c.lower | (lower_closure & self.seed_carrier),
            upper: c.upper & upper_closure & self.seed_carrier,
        };
        let definite_constraint = self
            .rules
            .iter()
            .zip(must)
            .position(|(r, on)| on && r.head.is_none() && r.positive & !lower_closure == 0);
        Ok(NarrowStep {
            before: c,
            lower_closure,
            upper_closure,
            after,
            inconsistent: after.lower & !after.upper != 0,
            definite_constraint,
        })
    }
    /// Independent baseline: enumerate FULL interpretations and use sets,
    /// rather than the seed projection or mask-based positive closure.
    #[must_use]
    pub fn direct_full_candidate_models(&self) -> Vec<Mask> {
        let mut models = Vec::new();
        for candidate_mask in subsets(self.universe) {
            let candidate: BTreeSet<usize> = (0..self.atoms)
                .filter(|a| candidate_mask & (1u64 << a) != 0)
                .collect();
            let mut reduct = Vec::new();
            let mut constraints = Vec::new();
            for r in &self.rules {
                let negative_holds = (0..self.atoms)
                    .all(|a| r.negative & (1u64 << a) == 0 || !candidate.contains(&a));
                let double_holds = (0..self.atoms)
                    .all(|a| r.double_negative & (1u64 << a) == 0 || candidate.contains(&a));
                if !negative_holds
                    || !double_holds
                    || (r.choice && r.head.is_some_and(|head| !candidate.contains(&head)))
                {
                    continue;
                }
                let body: BTreeSet<_> = (0..self.atoms)
                    .filter(|a| r.positive & (1u64 << a) != 0)
                    .collect();
                if let Some(h) = r.head {
                    reduct.push((h, body));
                } else {
                    constraints.push(body);
                }
            }
            let mut least = BTreeSet::new();
            loop {
                let mut next = least.clone();
                for (head, body) in &reduct {
                    if body.is_subset(&least) {
                        next.insert(*head);
                    }
                }
                if next == least {
                    break;
                }
                least = next;
            }
            if least == candidate && constraints.iter().all(|b| !b.is_subset(&least)) {
                models.push(candidate_mask);
            }
        }
        models.sort_unstable();
        models
    }
}

/// Lazy subset enumeration: it does not allocate an exponential vector.
pub fn subsets(carrier: Mask) -> impl Iterator<Item = Mask> {
    let mut next = Some(carrier);
    std::iter::from_fn(move || {
        let current = next?;
        next = if current == 0 {
            None
        } else {
            Some((current - 1) & carrier)
        };
        Some(current)
    })
}

/// Coverage disposition of one recorded seed cube.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LedgerAction {
    /// The cube still has unaccounted work.
    Pending,
    /// Its final narrowing step excludes every stable seed.
    Rejected,
    /// A singleton seed has a verified stable closure.
    Accepted {
        /// Total seed assignment on the program's seed carrier.
        seed: Mask,
        /// Full stable closure, including atoms outside the seed carrier.
        model: Mask,
    },
    /// An unknown atom partitions the remaining cube into two disjoint children.
    Split {
        /// Dense atom identifier on which the cube is split.
        atom: usize,
        /// Ledger index of the child where the atom is absent.
        false_child: usize,
        /// Ledger index of the child where the atom is present.
        true_child: usize,
    },
}
/// Input cube, replayable narrowing trace and final coverage disposition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LedgerNode {
    /// Original cube represented by this node.
    pub input: Cube,
    /// Complete narrowing trace, in execution order.
    pub steps: Vec<NarrowStep>,
    /// Remaining work or the reason this node has been accounted for.
    pub action: LedgerAction,
}
/// Counted ledger operations; the trace, rather than counts, certifies coverage.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LedgerCounters {
    /// Number of recorded cube nodes.
    pub nodes: u64,
    /// Number of recorded narrowing steps.
    pub transforms: u64,
    /// Number of binary cube partitions.
    pub splits: u64,
    /// Number of refuted cube leaves.
    pub rejected: u64,
    /// Number of stable singleton leaves.
    pub accepted: u64,
}
/// Exhaustive stable models with their replayable seed-coverage ledger.
#[derive(Clone, Debug)]
pub struct SearchResult {
    /// Full stable models in increasing mask order.
    pub models: Vec<Mask>,
    /// All cube nodes, with the root at index zero.
    pub ledger: Vec<LedgerNode>,
    /// Operation totals reproduced by [`verify_ledger`].
    pub counters: LedgerCounters,
}

/// Enumerate the full seed carrier by narrowing and disjoint binary splits.
///
/// # Errors
/// Returns an error if a generated cube fails validation or a narrowed singleton
/// fails exact reduct checking. Search is exhaustive and has no budget parameter.
pub fn exact_cube_search(p: &Program) -> Result<SearchResult> {
    fn visit(p: &Program, input: Cube, result: &mut SearchResult) -> Result<usize> {
        let index = result.ledger.len();
        result.ledger.push(LedgerNode {
            input,
            steps: Vec::new(),
            action: LedgerAction::Pending,
        });
        result.counters.nodes += 1;
        let mut cube = input;
        loop {
            let step = p.narrow(cube)?;
            result.counters.transforms += 1;
            let rejected = step.rejected();
            let next = step.after;
            result.ledger[index].steps.push(step);
            if rejected {
                result.ledger[index].action = LedgerAction::Rejected;
                result.counters.rejected += 1;
                return Ok(index);
            }
            if next == cube {
                break;
            }
            cube = next;
        }
        if cube.lower == cube.upper {
            let checked = p.check_seed(cube.lower)?;
            if !checked.stable {
                return Err("internal error: narrowed singleton is not stable".into());
            }
            result.ledger[index].action = LedgerAction::Accepted {
                seed: cube.lower,
                model: checked.closure,
            };
            result.models.push(checked.closure);
            result.counters.accepted += 1;
        } else {
            let atom = (cube.upper & !cube.lower).trailing_zeros() as usize;
            let bit = 1u64 << atom;
            let false_child = visit(
                p,
                Cube {
                    lower: cube.lower,
                    upper: cube.upper & !bit,
                },
                result,
            )?;
            let true_child = visit(
                p,
                Cube {
                    lower: cube.lower | bit,
                    upper: cube.upper,
                },
                result,
            )?;
            result.ledger[index].action = LedgerAction::Split {
                atom,
                false_child,
                true_child,
            };
            result.counters.splits += 1;
        }
        Ok(index)
    }
    let mut result = SearchResult {
        models: Vec::new(),
        ledger: Vec::new(),
        counters: LedgerCounters::default(),
    };
    visit(
        p,
        Cube {
            lower: 0,
            upper: p.seed_carrier,
        },
        &mut result,
    )?;
    result.models.sort_unstable();
    Ok(result)
}

/// Replay one ledger subtree with the exact cube its parent assigned to it.
fn verify_node(
    p: &Program,
    r: &SearchResult,
    index: usize,
    expected: Cube,
    visited: &mut BTreeSet<usize>,
    models: &mut Vec<Mask>,
    counts: &mut LedgerCounters,
) -> Result<()> {
    if !visited.insert(index) {
        return Err("ledger reuses a node or contains a cycle".into());
    }
    let node = r.ledger.get(index).ok_or("ledger child missing")?;
    if node.input != expected {
        return Err("ledger child does not cover expected cube".into());
    }
    counts.nodes += 1;
    let mut cube = expected;
    let mut rejected = false;
    if node.steps.is_empty() {
        return Err("ledger node has no checked transform".into());
    }
    for step in &node.steps {
        if rejected {
            return Err("ledger continues after rejection".into());
        }
        if *step != p.narrow(cube)? {
            return Err("ledger transform fails replay".into());
        }
        rejected = step.rejected();
        cube = step.after;
        counts.transforms += 1;
    }
    match node.action {
        LedgerAction::Pending => return Err("ledger has unresolved work".into()),
        LedgerAction::Rejected => {
            if !rejected {
                return Err("rejection lacks a refutation".into());
            }
            counts.rejected += 1;
        }
        LedgerAction::Accepted { seed, model } => {
            if rejected || cube.lower != cube.upper || cube.lower != seed {
                return Err("accepted leaf is not a feasible singleton".into());
            }
            let checked = p.check_seed(seed)?;
            if !checked.stable || checked.closure != model {
                return Err("invalid model certificate".into());
            }
            counts.accepted += 1;
            models.push(model);
        }
        LedgerAction::Split {
            atom,
            false_child,
            true_child,
        } => {
            if rejected || atom >= p.atoms {
                return Err("invalid split".into());
            }
            let bit = 1u64 << atom;
            if cube.upper & !cube.lower & bit == 0 {
                return Err("split variable is not unknown".into());
            }
            counts.splits += 1;
            verify_node(
                p,
                r,
                false_child,
                Cube {
                    lower: cube.lower,
                    upper: cube.upper & !bit,
                },
                visited,
                models,
                counts,
            )?;
            verify_node(
                p,
                r,
                true_child,
                Cube {
                    lower: cube.lower | bit,
                    upper: cube.upper,
                },
                visited,
                models,
                counts,
            )?;
        }
    }
    Ok(())
}

/// Replays every recorded transform and verifies exact split coverage. Counters
/// alone are never used as a completeness certificate.
///
/// # Errors
/// Returns an error for invalid or reused nodes, incorrect narrowing, unresolved
/// work, invalid model certificates, nonpartitioning splits, or mismatched totals.
pub fn verify_ledger(p: &Program, result: &SearchResult) -> Result<()> {
    let mut visited = BTreeSet::new();
    let mut models = Vec::new();
    let mut counts = LedgerCounters::default();
    verify_node(
        p,
        result,
        0,
        Cube {
            lower: 0,
            upper: p.seed_carrier,
        },
        &mut visited,
        &mut models,
        &mut counts,
    )?;
    if visited.len() != result.ledger.len() {
        return Err("ledger contains unaccounted nodes".into());
    }
    models.sort_unstable();
    if models != result.models || counts != result.counters {
        return Err("ledger outputs or counters disagree".into());
    }
    Ok(())
}

#[derive(Clone, Copy, Debug)]
enum Event {
    Activate {
        epoch: u64,
        atom: usize,
    },
    Body {
        epoch: u64,
        rule: usize,
        atom: usize,
    },
}
/// Quiescent event execution with exact closure and delivery accounting.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct EventResult {
    /// Atoms whose activation was delivered during this epoch.
    pub closure: Mask,
    /// Exact seed agreement after delivery and constraint checks.
    pub stable: bool,
    /// Whether an active constraint fired.
    pub constraint_violated: bool,
    /// Distinct atom activations propagated to their consumers.
    pub atom_emissions: u64,
    /// Unique positive antecedent deliveries.
    pub body_deliveries: u64,
    /// Active rules whose complete positive bodies were delivered.
    pub rule_firings: u64,
}
struct EventEngine<'a> {
    p: &'a Program,
    seed: Mask,
    epoch: u64,
    active: Vec<bool>,
    remaining: Vec<u32>,
    seen: Vec<Mask>,
    fired: Vec<bool>,
    consumers: Vec<Vec<usize>>,
    queue: Vec<Event>,
    result: EventResult,
}

impl<'a> EventEngine<'a> {
    fn new(p: &'a Program, seed: Mask, epoch: u64) -> Result<Self> {
        p.validate_seed(seed)?;
        let active: Vec<_> = p.rules.iter().map(|r| Program::enabled(r, seed)).collect();
        let mut consumers = vec![Vec::new(); p.atoms];
        for (i, r) in p.rules.iter().enumerate() {
            if active[i] {
                for (a, list) in consumers.iter_mut().enumerate() {
                    if r.positive & (1u64 << a) != 0 {
                        list.push(i);
                    }
                }
            }
        }
        let mut engine = Self {
            p,
            seed,
            epoch,
            active,
            remaining: p.rules.iter().map(|r| r.positive.count_ones()).collect(),
            seen: vec![0; p.rules.len()],
            fired: vec![false; p.rules.len()],
            consumers,
            queue: Vec::new(),
            result: EventResult::default(),
        };
        for i in 0..p.rules.len() {
            if engine.active[i] && engine.remaining[i] == 0 {
                engine.fire(i)?;
            }
        }
        Ok(engine)
    }
    fn fire(&mut self, rule: usize) -> Result<()> {
        if self.fired[rule] {
            return Err("rule fired twice".into());
        }
        self.fired[rule] = true;
        self.result.rule_firings += 1;
        if let Some(atom) = self.p.rules[rule].head {
            self.queue.push(Event::Activate {
                epoch: self.epoch,
                atom,
            });
        } else {
            self.result.constraint_violated = true;
        }
        Ok(())
    }
    fn handle(&mut self, event: Event) -> Result<()> {
        let epoch = match event {
            Event::Activate { epoch, .. } | Event::Body { epoch, .. } => epoch,
        };
        if epoch != self.epoch {
            return Err("stale epoch event".into());
        }
        match event {
            Event::Activate { atom, .. } => {
                if atom >= self.p.atoms {
                    return Err("activation outside carrier".into());
                }
                let bit = 1u64 << atom;
                if self.result.closure & bit != 0 {
                    return Ok(());
                }
                self.result.closure |= bit;
                self.result.atom_emissions += 1;
                for &rule in &self.consumers[atom] {
                    self.queue.push(Event::Body {
                        epoch: self.epoch,
                        rule,
                        atom,
                    });
                }
            }
            Event::Body { rule, atom, .. } => {
                if rule >= self.p.rules.len() || atom >= self.p.atoms || !self.active[rule] {
                    return Err("body delivery outside active graph".into());
                }
                let bit = 1u64 << atom;
                if self.p.rules[rule].positive & bit == 0 {
                    return Err("foreign antecedent".into());
                }
                if self.seen[rule] & bit != 0 {
                    return Err("duplicate antecedent delivery".into());
                }
                if self.remaining[rule] == 0 {
                    return Err("body counter underflow".into());
                }
                self.seen[rule] |= bit;
                self.remaining[rule] -= 1;
                self.result.body_deliveries += 1;
                if self.remaining[rule] == 0 {
                    self.fire(rule)?;
                }
            }
        }
        Ok(())
    }
    fn finish(mut self) -> Result<EventResult> {
        if !self.queue.is_empty() {
            return Err("acceptance attempted before quiescence".into());
        }
        // Explicit reference-only coverage audit. A device backend must provide
        // its own reliable delivery/quiescence argument; empty queues alone do
        // not certify that a message was never lost.
        for (i, r) in self.p.rules.iter().enumerate() {
            if !self.active[i] {
                continue;
            }
            if self.seen[i] != r.positive & self.result.closure {
                return Err("lost antecedent delivery".into());
            }
            let ready = r.positive & !self.result.closure == 0;
            if ready != self.fired[i] {
                return Err("ready rule was not fired".into());
            }
            if self.fired[i]
                && r.head
                    .is_some_and(|h| self.result.closure & (1u64 << h) == 0)
            {
                return Err("lost head activation".into());
            }
        }
        self.result.stable = self.result.closure & self.p.seed_carrier == self.seed
            && !self.result.constraint_violated;
        Ok(self.result)
    }
    fn run(mut self, schedule_seed: u64) -> Result<EventResult> {
        let mut rng = Rng::new(schedule_seed);
        while !self.queue.is_empty() {
            let index = rng.index(self.queue.len())?;
            let event = self.queue.swap_remove(index);
            self.handle(event)?;
        }
        self.finish()
    }
}

/// Compute reduct closure using a deterministic shuffled event schedule.
///
/// # Errors
/// Returns an error for an invalid seed, invalid/duplicate/stale delivery,
/// missing delivery at quiescence, or an unrepresentable queue index.
pub fn event_closure(p: &Program, seed: Mask, schedule_seed: u64) -> Result<EventResult> {
    EventEngine::new(p, seed, 1)?.run(schedule_seed)
}

#[derive(Clone)]
struct Rng(u64);
// Xorshift cannot leave zero; preserve the original deterministic replacement.
const NONZERO_RNG_SEED: u64 = 0x9e37_79b9_7f4a_7c15;
impl Rng {
    fn new(seed: u64) -> Self {
        Self(if seed == 0 { NONZERO_RNG_SEED } else { seed })
    }
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn below(&mut self, upper: u64) -> u64 {
        self.next() % upper
    }
    fn index(&mut self, upper: usize) -> Result<usize> {
        let bound = u64::try_from(upper).map_err(|_| "index bound exceeds generator carrier")?;
        // The remainder is below the original usize bound. Keep that conversion
        // checked so its invariant remains explicit on every target width.
        usize::try_from(self.below(bound))
            .map_err(|_| "generated index exceeds host carrier".into())
    }
}

/// Executed observations from the fixed finite semantic validation campaign.
#[derive(Clone, Debug, Default)]
pub struct ValidationCounts {
    /// One-atom rule subsets exhaustively checked.
    pub exhaustive_one_atom_programs: u64,
    /// Two-atom rule subsets exhaustively checked.
    pub exhaustive_two_atom_programs: u64,
    /// Programs produced by the fixed random generator.
    pub random_programs: u64,
    /// Total programs checked across all three populations.
    pub programs: u64,
    /// Full interpretations visited by the independent baseline.
    pub full_candidates: u64,
    /// Total seed assignments checked by reduct closure.
    pub seeds: u64,
    /// Complete shuffled event executions compared with reduct closure.
    pub event_schedules: u64,
    /// Seed cubes checked for enclosure and narrowing.
    pub cubes: u64,
    /// Seed completions checked against lower and upper closure bounds.
    pub enclosure_checks: u64,
    /// Stable seed completions checked for survival under narrowing.
    pub stable_seed_survival_checks: u64,
    /// Search ledger nodes replayed across all programs.
    pub ledger_nodes: u64,
    /// Full stable models found across all programs.
    pub models: u64,
}
impl ValidationCounts {
    /// Render the legacy campaign report with the supplied elapsed seconds.
    /// The caller supplies a finite nonnegative duration for valid JSON metadata.
    #[must_use]
    pub fn json(&self, seconds: f64) -> String {
        format!(
            concat!(
                "{{\n  \"status\": \"PASS\",\n  \"implementation\": \"finite CPU semantic reference\",\n",
                "  \"random_seed\": 20260905,\n  \"random_atoms_at_most\": 5,\n",
                "  \"random_rules_at_most\": 20,\n  \"event_orders_per_seed\": 3,\n",
                "  \"seconds\": {:.3},\n  \"time_scope\": \"ground_kernel_campaign\",\n  \"counts\": {{\n",
                "    \"exhaustive_one_atom_programs\": {},\n    \"exhaustive_two_atom_programs\": {},\n",
                "    \"random_programs\": {},\n    \"programs\": {},\n    \"full_candidates\": {},\n",
                "    \"seeds\": {},\n    \"event_schedules\": {},\n    \"cubes\": {},\n",
                "    \"enclosure_checks\": {},\n    \"stable_seed_survival_checks\": {},\n",
                "    \"ledger_nodes\": {},\n    \"models\": {}\n  }}\n}}\n"
            ),
            seconds,
            self.exhaustive_one_atom_programs,
            self.exhaustive_two_atom_programs,
            self.random_programs,
            self.programs,
            self.full_candidates,
            self.seeds,
            self.event_schedules,
            self.cubes,
            self.enclosure_checks,
            self.stable_seed_survival_checks,
            self.ledger_nodes,
            self.models
        )
    }
}

fn all_cubes(carrier: Mask) -> Vec<Cube> {
    let mut cubes = vec![Cube { lower: 0, upper: 0 }];
    for atom in 0..64 {
        let bit = 1u64 << atom;
        if carrier & bit == 0 {
            continue;
        }
        let old = std::mem::take(&mut cubes);
        for c in old {
            cubes.push(c);
            cubes.push(Cube {
                lower: c.lower,
                upper: c.upper | bit,
            });
            cubes.push(Cube {
                lower: c.lower | bit,
                upper: c.upper | bit,
            });
        }
    }
    cubes
}

fn require(condition: bool, message: &str, p: &Program) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(format!("{message}; counterexample: {p:?}"))
    }
}

fn validate_program(p: &Program, counts: &mut ValidationCounts) -> Result<()> {
    let direct = p.direct_full_candidate_models();
    let mut checks = Vec::new();
    let mut projected_models = Vec::new();
    for seed in subsets(p.seed_carrier) {
        let checked = p.check_seed(seed)?;
        if checked.stable {
            projected_models.push(checked.closure);
        }
        for order in [1, 0x0123_4567_89ab_cdef, 0xf00d_0000 ^ counts.programs] {
            let event = event_closure(p, seed, order)?;
            require(
                event.closure == checked.closure
                    && event.stable == checked.stable
                    && event.constraint_violated == checked.constraint_violated,
                "event/reduct disagreement",
                p,
            )?;
            require(
                event.atom_emissions == u64::from(event.closure.count_ones()),
                "atom emitted more than once",
                p,
            )?;
            counts.event_schedules += 1;
        }
        checks.push((seed, checked));
        counts.seeds += 1;
    }
    projected_models.sort_unstable();
    require(
        projected_models == direct,
        "projected fixed-point theorem failure",
        p,
    )?;
    for cube in all_cubes(p.seed_carrier) {
        let step = p.narrow(cube)?;
        require(
            step.lower_closure & !step.upper_closure == 0,
            "closure bounds reversed",
            p,
        )?;
        for &(seed, checked) in &checks {
            if cube.lower & !seed != 0 || seed & !cube.upper != 0 {
                continue;
            }
            require(
                step.lower_closure & !checked.closure == 0
                    && checked.closure & !step.upper_closure == 0,
                "cube does not enclose a completion",
                p,
            )?;
            counts.enclosure_checks += 1;
            if checked.stable {
                require(
                    !step.rejected()
                        && step.after.lower & !seed == 0
                        && seed & !step.after.upper == 0,
                    "narrowing rejected a stable seed",
                    p,
                )?;
                counts.stable_seed_survival_checks += 1;
            }
        }
        counts.cubes += 1;
    }
    let search = exact_cube_search(p)?;
    verify_ledger(p, &search)?;
    require(
        search.models == direct,
        "cube search differs from full reference",
        p,
    )?;
    counts.ledger_nodes += search.counters.nodes;
    counts.models += direct.len() as u64;
    counts.programs += 1;
    counts.full_candidates += 1u64 << p.atoms;
    Ok(())
}

fn rule_universe(atoms: usize) -> Vec<Rule> {
    let mut rules = Vec::new();
    for kind in 0..=(2 * atoms) {
        let head = if kind == 0 {
            None
        } else {
            Some((kind - 1) % atoms)
        };
        let choice = kind > atoms;
        for positive in 0..(1u64 << atoms) {
            for negative in 0..(1u64 << atoms) {
                for double_negative in 0..(1u64 << atoms) {
                    rules.push(Rule {
                        head,
                        choice,
                        positive,
                        negative,
                        double_negative,
                    });
                }
            }
        }
    }
    rules
}

fn exhaustive(atoms: usize, max_rules: usize, counts: &mut ValidationCounts) -> Result<u64> {
    fn combinations(
        atoms: usize,
        universe: &[Rule],
        start: usize,
        remaining: usize,
        chosen: &mut Vec<Rule>,
        counts: &mut ValidationCounts,
    ) -> Result<()> {
        if remaining == 0 {
            return validate_program(&Program::new(atoms, chosen.clone())?, counts);
        }
        if universe.len() - start < remaining {
            return Ok(());
        }
        for i in start..=(universe.len() - remaining) {
            chosen.push(universe[i]);
            combinations(atoms, universe, i + 1, remaining - 1, chosen, counts)?;
            chosen.pop();
        }
        Ok(())
    }
    let before = counts.programs;
    let universe = rule_universe(atoms);
    for size in 0..=max_rules {
        combinations(atoms, &universe, 0, size, &mut Vec::new(), counts)?;
    }
    Ok(counts.programs - before)
}

/// All one-atom programs with <=4 distinct rules, all two-atom programs with
/// <=2 distinct rules, then 1000 reproducible random programs with <=5 atoms.
///
/// # Errors
/// Returns the first admission, execution or semantic-agreement failure, with
/// the finite counterexample when the failed comparison has one.
pub fn run_validation() -> Result<ValidationCounts> {
    let mut counts = ValidationCounts::default();
    counts.exhaustive_one_atom_programs = exhaustive(1, 4, &mut counts)?;
    counts.exhaustive_two_atom_programs = exhaustive(2, 2, &mut counts)?;
    let mut rng = Rng::new(20_260_905);
    for sample in 0..1000 {
        let atoms = 1 + rng.index(5)?;
        let size = rng.index(21)?;
        let mut rules = Vec::new();
        for _ in 0..size {
            let kind = rng.below(4);
            let head = if kind == 3 {
                None
            } else {
                Some(rng.index(atoms)?)
            };
            let mut mask = || {
                if sample % 2 == 1 {
                    rng.below(1u64 << atoms)
                } else {
                    (0..atoms).fold(0, |m, a| {
                        m | if rng.below(100) < 22 { 1u64 << a } else { 0 }
                    })
                }
            };
            rules.push(Rule {
                head,
                choice: kind == 2,
                positive: mask(),
                negative: mask(),
                double_negative: mask(),
            });
        }
        validate_program(&Program::new(atoms, rules)?, &mut counts)?;
        counts.random_programs += 1;
    }
    Ok(counts)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn p(n: usize, rules: Vec<Rule>) -> Program {
        Program::new(n, rules).unwrap()
    }
    fn fact(a: usize) -> Rule {
        Rule::normal(a, 0, 0, 0)
    }

    #[test]
    fn adversarial_semantics_table() {
        let examples = vec![
            (p(0, vec![]), vec![0]),
            (p(0, vec![Rule::constraint(0, 0, 0)]), vec![]),
            (p(1, vec![Rule::normal(0, 1, 0, 0)]), vec![0]),
            (p(1, vec![Rule::normal(0, 0, 1, 0)]), vec![]),
            (
                p(2, vec![Rule::normal(0, 0, 2, 0), Rule::normal(1, 0, 1, 0)]),
                vec![1, 2],
            ),
            (
                p(
                    3,
                    vec![
                        Rule::singleton_choice(0, 0, 0),
                        Rule::normal(1, 1, 0, 0),
                        Rule::normal(2, 4, 0, 0),
                        Rule::constraint(0, 2, 0),
                    ],
                ),
                vec![3],
            ),
            (p(1, vec![Rule::normal(0, 0, 1, 1)]), vec![0]),
            (
                p(
                    2,
                    vec![fact(0), Rule::normal(1, 1, 0, 0), Rule::constraint(0, 2, 0)],
                ),
                vec![3],
            ),
            (p(1, vec![Rule::normal(0, 0, 0, 1)]), vec![0, 1]),
        ];
        for (program, expected) in examples {
            assert_eq!(program.direct_full_candidate_models(), expected);
            let found = exact_cube_search(&program).unwrap();
            assert_eq!(found.models, expected);
            verify_ledger(&program, &found).unwrap();
        }
    }
    #[test]
    fn invalid_carriers_are_rejected_without_shifts_overflowing() {
        assert!(Program::new(64, vec![]).is_err());
        assert!(Program::new(1000, vec![]).is_err());
        assert!(Program::new(63, vec![]).is_ok());
        assert!(Program::new(1, vec![fact(1)]).is_err());
        assert!(Program::new(1, vec![Rule::normal(0, 2, 0, 0)]).is_err());
        let mut invalid = Rule::constraint(0, 0, 0);
        invalid.choice = true;
        assert!(Program::new(1, vec![invalid]).is_err());
        let program = p(1, vec![]);
        assert!(program.check_seed(1).is_err());
        assert!(program.narrow(Cube { lower: 1, upper: 0 }).is_err());
    }
    #[test]
    fn depth_multiple_supports_and_positive_cycles() {
        let mut rules = vec![fact(0), fact(0)];
        for a in 1..6 {
            rules.push(Rule::normal(a, 1u64 << (a - 1), 0, 0));
        }
        rules.push(Rule::normal(0, 1u64 << 5, 0, 0));
        let program = p(6, rules);
        for order in 0..30 {
            let result = event_closure(&program, 0, order).unwrap();
            assert_eq!(result.closure, 63);
            assert_eq!(result.atom_emissions, 6);
            assert!(result.stable);
        }
    }
    #[test]
    fn upper_closure_must_not_be_an_incomplete_forward_prefix() {
        let program = p(
            2,
            vec![fact(0), Rule::normal(1, 1, 0, 0), Rule::constraint(0, 2, 0)],
        );
        let step = program.narrow(Cube { lower: 0, upper: 2 }).unwrap();
        assert_eq!(step.upper_closure, 3);
        assert_eq!(step.after, Cube { lower: 2, upper: 2 });
        let unsound_partial_upper = 1;
        assert_ne!(
            program.check_seed(2).unwrap().closure & !unsound_partial_upper,
            0
        );
    }
    #[test]
    fn upper_closure_can_combine_incompatible_gates() {
        let program = p(
            4,
            vec![
                Rule::normal(0, 0, 2, 0),
                Rule::normal(2, 0, 0, 2),
                Rule::normal(3, 1 | 4, 0, 0),
            ],
        );
        let step = program.narrow(Cube { lower: 0, upper: 2 }).unwrap();
        assert_ne!(step.upper_closure & 8, 0);
        for seed in [0, 2] {
            assert_eq!(program.check_seed(seed).unwrap().closure & 8, 0);
        }
    }
    #[test]
    fn stale_epochs_fail_closed() {
        let program = p(1, vec![fact(0)]);
        let mut engine = EventEngine::new(&program, 0, 2).unwrap();
        let err = engine
            .handle(Event::Activate { epoch: 1, atom: 0 })
            .unwrap_err();
        assert!(err.contains("stale epoch"));
    }
    #[test]
    fn duplicate_antecedents_are_detected() {
        let program = p(2, vec![Rule::normal(1, 1, 0, 0)]);
        let mut engine = EventEngine::new(&program, 0, 1).unwrap();
        let event = Event::Body {
            epoch: 1,
            rule: 0,
            atom: 0,
        };
        engine.handle(event).unwrap();
        assert!(engine.handle(event).unwrap_err().contains("duplicate"));
    }
    #[test]
    fn counter_underflow_is_detected() {
        let program = p(2, vec![Rule::normal(1, 1, 0, 0)]);
        let mut engine = EventEngine::new(&program, 0, 1).unwrap();
        engine.remaining[0] = 0;
        assert!(
            engine
                .handle(Event::Body {
                    epoch: 1,
                    rule: 0,
                    atom: 0
                })
                .unwrap_err()
                .contains("underflow")
        );
    }
    #[test]
    fn missing_events_and_premature_acceptance_are_detected() {
        let program = p(2, vec![fact(0), Rule::normal(1, 1, 0, 0)]);
        assert!(
            EventEngine::new(&program, 0, 1)
                .unwrap()
                .finish()
                .unwrap_err()
                .contains("quiescence")
        );
        let mut engine = EventEngine::new(&program, 0, 1).unwrap();
        engine.queue.clear();
        assert!(engine.finish().unwrap_err().contains("lost head"));
        let mut engine = EventEngine::new(&program, 0, 1).unwrap();
        let first = engine.queue.pop().unwrap();
        engine.handle(first).unwrap();
        engine.queue.clear();
        assert!(engine.finish().unwrap_err().contains("lost antecedent"));
    }
    #[test]
    fn a_new_candidate_cannot_keep_old_closure() {
        let program = p(
            2,
            vec![Rule::singleton_choice(0, 0, 0), Rule::normal(1, 1, 0, 0)],
        );
        assert_eq!(event_closure(&program, 1, 1).unwrap().closure, 3);
        assert_eq!(event_closure(&program, 0, 1).unwrap().closure, 0);
    }
    #[test]
    fn ledger_replay_rejects_corruption_and_missing_work() {
        let program = p(1, vec![Rule::singleton_choice(0, 0, 0)]);
        let result = exact_cube_search(&program).unwrap();
        verify_ledger(&program, &result).unwrap();
        let mut corrupt = result.clone();
        corrupt.models.clear();
        assert!(verify_ledger(&program, &corrupt).is_err());
        let mut pending = result.clone();
        pending.ledger[0].action = LedgerAction::Pending;
        assert!(verify_ledger(&program, &pending).is_err());
        let mut lost = result.clone();
        lost.ledger.pop();
        assert!(verify_ledger(&program, &lost).is_err());
        let mut bad_transform = result;
        bad_transform.ledger[0].steps[0].upper_closure ^= 1;
        assert!(verify_ledger(&program, &bad_transform).is_err());
    }
    #[test]
    fn exhaustive_and_seeded_random_implementations_agree() {
        let counts = run_validation().unwrap();
        assert_eq!(counts.exhaustive_one_atom_programs, 12_951);
        assert_eq!(counts.exhaustive_two_atom_programs, 51_361);
        assert_eq!(counts.random_programs, 1_000);
        assert_eq!(counts.programs, 65_312);
    }
}
