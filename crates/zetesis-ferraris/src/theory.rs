use std::fmt;
use std::sync::Arc;

/// A raw consecutive row in the theory's sole operand arena.
/// Admission checks addition, containment, arity and every referenced child.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OperandSpan {
    /// First arena cell, including no implicit offset from the node index.
    pub start: usize,
    /// Number of ordered occurrences; stored wide groups require at least three.
    pub length: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Storage {
    Atom(usize),
    False,
    Implies(usize, usize),
    AndPair([usize; 2]),
    OrPair([usize; 2]),
    AndSpan(OperandSpan),
    OrSpan(OperandSpan),
}

/// One raw formula node. Child IDs are absolute preceding-node indices.
///
/// A pair stores its two cells inline; a wider conjunction or disjunction stores
/// a span into the paired owner's arena. Raw constructors do not establish
/// admission. Equality compares storage, including span addresses; semantic
/// interning must compare [`NodeView`], whose slices compare operand contents.
/// Default negation remains implication to falsum, not a reduct complement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(transparent)]
pub struct Node(Storage);

impl Node {
    /// An unchecked atom in the theory's finite universe.
    #[must_use]
    pub const fn atom(atom: usize) -> Self {
        Self(Storage::Atom(atom))
    }

    /// Falsum.
    #[must_use]
    pub const fn falsum() -> Self {
        Self(Storage::False)
    }

    /// Classical implication between two unchecked preceding-node IDs.
    #[must_use]
    pub const fn implies(left: usize, right: usize) -> Self {
        Self(Storage::Implies(left, right))
    }

    /// The canonical raw representation of a two-operand conjunction.
    #[must_use]
    pub const fn and_pair(operands: [usize; 2]) -> Self {
        Self(Storage::AndPair(operands))
    }

    /// The canonical raw representation of a two-operand disjunction.
    #[must_use]
    pub const fn or_pair(operands: [usize; 2]) -> Self {
        Self(Storage::OrPair(operands))
    }

    /// An unchecked wide conjunction. Admission rejects lengths below three.
    #[must_use]
    pub const fn and_span(span: OperandSpan) -> Self {
        Self(Storage::AndSpan(span))
    }

    /// An unchecked wide disjunction. Admission rejects lengths below three.
    #[must_use]
    pub const fn or_span(span: OperandSpan) -> Self {
        Self(Storage::OrSpan(span))
    }

    fn occurrences(self) -> usize {
        match self.0 {
            Storage::Atom(_) | Storage::False => 0,
            Storage::Implies(_, _) | Storage::AndPair(_) | Storage::OrPair(_) => 2,
            Storage::AndSpan(span) | Storage::OrSpan(span) => span.length,
        }
    }

    #[inline]
    fn view<'a>(&'a self, operands: &'a [usize]) -> Result<NodeView<'a>, AdmissionError> {
        match &self.0 {
            Storage::Atom(atom) => Ok(NodeView::Atom(*atom)),
            Storage::False => Ok(NodeView::False),
            Storage::Implies(left, right) => Ok(NodeView::Implies(*left, *right)),
            Storage::AndPair(pair) => Ok(NodeView::And(pair)),
            Storage::OrPair(pair) => Ok(NodeView::Or(pair)),
            Storage::AndSpan(span) => Ok(NodeView::And(read_span(operands, *span)?)),
            Storage::OrSpan(span) => Ok(NodeView::Or(read_span(operands, *span)?)),
        }
    }

    fn rebase(self, offset: usize) -> Result<Self, AdmissionError> {
        let rebase = |span: OperandSpan| -> Result<OperandSpan, AdmissionError> {
            Ok(OperandSpan {
                start: span.start.checked_sub(offset).ok_or(AdmissionError::Span)?,
                length: span.length,
            })
        };
        Ok(match self.0 {
            Storage::AndSpan(span) => Self::and_span(rebase(span)?),
            Storage::OrSpan(span) => Self::or_span(rebase(span)?),
            _ => self,
        })
    }
}

/// Storage-independent formula meaning, borrowing a complete ordered child row.
/// Pairs and wider groups use the same slice and preserve repeated occurrences.
/// Implication is intrinsically binary. Raw construction may present empty or
/// singleton groups to an owned builder, which normalizes them before storage.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NodeView<'a> {
    /// An atom in the declared finite universe.
    Atom(usize),
    /// Falsum.
    False,
    /// Classical implication, antecedent then consequent.
    Implies(usize, usize),
    /// Ordered conjuncts.
    And(&'a [usize]),
    /// Ordered disjuncts.
    Or(&'a [usize]),
}

impl NodeView<'_> {
    /// Read the node's shape without retaining its borrowed operand row.
    pub(crate) fn is_false(self) -> bool {
        match self {
            Self::False => true,
            Self::Atom(_) | Self::Implies(..) | Self::And(_) | Self::Or(_) => false,
        }
    }
}

/// A borrow of the paired node and operand buffers, without admission evidence.
/// Both inline and arena-backed rows borrow this same immutable owner.
#[derive(Clone, Copy, Debug)]
pub struct FormulaView<'a> {
    nodes: &'a [Node],
    operands: &'a [usize],
}
impl<'a> FormulaView<'a> {
    /// Number of stored nodes; constant time.
    #[must_use]
    pub const fn len(self) -> usize {
        self.nodes.len()
    }

    /// Whether there are no stored nodes; constant time.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.nodes.is_empty()
    }

    /// Borrow one logical node, checking raw storage before returning its row.
    /// This does not check atom IDs or the row's backward references.
    ///
    /// # Errors
    /// Refuses an absent node, a noncanonical span arity, overflow or a range
    /// outside the paired arena. Admitted [`Theory`] views cannot fail for an
    /// index below their node count.
    #[inline]
    pub fn node(self, index: usize) -> Result<NodeView<'a>, AdmissionError> {
        self.nodes
            .get(index)
            .ok_or(AdmissionError::Edge)?
            .view(self.operands)
    }
}

/// Raw formula buffers that always travel together.
///
/// The cached occurrence total counts every implication/pair edge and every
/// wide-span occurrence, including overlapping ranges. It is an arithmetic
/// quantity, not evidence of valid spans, topology or atom IDs. Read-only access
/// cannot invalidate it. Raw construction transfers both vectors without copying;
/// owned mutation is available only through a scoped append transaction. The
/// private cache supports bounded construction; theory admission independently
/// recounts the actual nodes and establishes its own occurrence bound.
#[derive(Debug, Default)]
pub struct FormulaParts {
    nodes: Vec<Node>,
    operands: Vec<usize>,
    occurrences: usize,
}
impl FormulaParts {
    /// Transfer raw buffers and count declared edge occurrences in O(nodes) time.
    /// No range, topology, atom-universe or logical truth claim is established.
    /// Unused arena cells and overlapping spans remain representable raw input.
    ///
    /// # Errors
    /// Refuses overflow in the total declared occurrences. No storage is allocated.
    pub fn new(nodes: Vec<Node>, operands: Vec<usize>) -> Result<Self, AdmissionError> {
        let occurrences = count_occurrences(&nodes)?;
        Ok(Self {
            nodes,
            operands,
            occurrences,
        })
    }

    /// Borrow the paired graph, including raw input not yet admitted.
    #[must_use]
    pub fn view(&self) -> FormulaView<'_> {
        FormulaView {
            nodes: &self.nodes,
            operands: &self.operands,
        }
    }

    /// Raw stored nodes; spans must be interpreted with [`Self::operands`].
    #[must_use]
    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }

    /// Raw arena cells. Inline pair cells are held by their nodes instead.
    #[must_use]
    pub fn operands(&self) -> &[usize] {
        &self.operands
    }

    /// Total logical edge occurrences, independent of physical layout.
    #[must_use]
    pub const fn occurrences(&self) -> usize {
        self.occurrences
    }

    /// Actual retained node capacity; no allocation or traversal.
    #[must_use]
    pub fn node_capacity(&self) -> usize {
        self.nodes.capacity()
    }

    /// Actual retained arena capacity; no allocation or traversal.
    #[must_use]
    pub fn operand_capacity(&self) -> usize {
        self.operands.capacity()
    }
}

/// Explicit inclusive bounds on an admitted finite formula DAG.
#[derive(Clone, Copy, Debug)]
pub struct AdmissionLimits {
    /// Maximum number of propositions, including unsupported atoms.
    pub max_atoms: usize,
    /// Maximum number of circuit nodes.
    pub max_nodes: usize,
    /// Maximum number of asserted root formulas.
    pub max_roots: usize,
    /// Maximum logical child occurrences, including inline pairs and implication.
    /// Also bounds raw arena length, so unused supplied cells are not free.
    /// Overlapping spans count once per referenced occurrence. Vector capacities
    /// remain caller-owned storage and are not bounded by these length ceilings.
    pub max_operands: usize,
}
impl Default for AdmissionLimits {
    fn default() -> Self {
        Self {
            max_atoms: 65_536,
            max_nodes: 1_048_576,
            max_roots: 262_144,
            max_operands: 2_097_152,
        }
    }
}

/// Invalid shape, foreign interpretation, or refused storage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdmissionError {
    /// An explicit finite shape bound or checked count was exceeded.
    Limit,
    /// An atom index lies outside the declared universe.
    Atom,
    /// A circuit edge is forward, cyclic, or out of range.
    Edge,
    /// A stored wide connective has fewer than three operands.
    Arity,
    /// An operand span overflows or falls outside its paired arena.
    Span,
    /// An asserted root is not a circuit node.
    Root,
    /// Formula or interpretation storage could not be reserved.
    Allocation,
}
impl fmt::Display for AdmissionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Limit => "formula admission limit exceeded",
            Self::Atom => "atom is outside the formula universe",
            Self::Edge => "formula edge must refer to a preceding node",
            Self::Arity => "stored wide formula requires at least three operands",
            Self::Span => "formula operand span is outside its arena",
            Self::Root => "formula root is outside the circuit",
            Self::Allocation => "formula storage could not be reserved",
        })
    }
}
impl std::error::Error for AdmissionError {}

/// A detached, self-owned append suffix, with absolute child IDs unchanged.
///
/// The first stored node formerly had index [`Self::first`]. Views use local
/// positions (zero is that first node); operands still reference original absolute
/// IDs. This is a remapping input, not an independently admitted theory. Only new
/// arena offsets are rebased, never child IDs. The source retains its prefix and
/// may append while this object is read independently.
#[derive(Debug)]
pub struct FormulaSuffix {
    first: usize,
    parts: FormulaParts,
}
impl FormulaSuffix {
    /// Former absolute index of this suffix's first node.
    #[must_use]
    pub const fn first(&self) -> usize {
        self.first
    }

    /// Borrow the paired suffix; node positions are local, child IDs absolute.
    #[must_use]
    pub fn view(&self) -> FormulaView<'_> {
        self.parts.view()
    }

    /// Inspect its owned storage and capacities for enclosing accounting.
    #[must_use]
    pub const fn parts(&self) -> &FormulaParts {
        &self.parts
    }
}

/// An append-only borrow with a private paired checkpoint.
///
/// Callers can append logical nodes and read the graph, but cannot replace the
/// owner, edit its prefix or inject raw arena spans. Thus every new wide row owns
/// fresh consecutive cells. Dropping without commit restores both original
/// lengths, occurrence total and validation frontier; reserved capacities survive.
/// Validation discovered inside a rolled-back append is discarded because an
/// unchecked raw prefix can refer to newly appended arena cells. Private offsets cannot be supplied from
/// another owner, from a future length, or reused after completion.
///
/// This controls storage, not work/cancellation: enclosing algorithms must charge
/// all visited nodes/operands and any detachment copy before these operations.
/// No failed allocation or limit check publishes a partial node or operand row.
#[derive(Debug)]
pub struct FormulaTransaction<'a> {
    parts: &'a mut FormulaParts,
    validated: &'a mut usize,
    first: usize,
    operand_start: usize,
    initial_validated: usize,
    occurrences: usize,
    completed: bool,
}
impl<'a> FormulaTransaction<'a> {
    pub(crate) fn new(parts: &'a mut FormulaParts, validated: &'a mut usize) -> Self {
        Self {
            first: parts.nodes.len(),
            operand_start: parts.operands.len(),
            initial_validated: *validated,
            occurrences: parts.occurrences,
            parts,
            validated,
            completed: false,
        }
    }

    /// Read the full current paired graph; constant time.
    #[must_use]
    pub fn view(&self) -> FormulaView<'_> {
        self.parts.view()
    }

    /// Inspect full current lengths, capacities and occurrence total.
    #[must_use]
    pub fn parts(&self) -> &FormulaParts {
        self.parts
    }

    /// Absolute first node index in this transaction; constant time.
    #[must_use]
    pub const fn first(&self) -> usize {
        self.first
    }

    /// Append one logical node, normalizing empty and singleton groups.
    ///
    /// A singleton returns its checked child. Empty OR appends falsum; empty AND
    /// appends falsum and its self-implication. A pair is inline and a wider group
    /// copies its ordered row once to fresh arena cells. Repetition is preserved.
    /// All supplied child IDs must precede this call; atom-universe admission is
    /// left to [`Theory::new`]. If the entire existing prefix has completed
    /// topology validation, successful appends extend that frontier: this call
    /// checks every child and constructs canonical storage. An unchecked prefix
    /// remains unchecked. No structural interning is performed here.
    ///
    /// Costs O(1 + arity) work and at most one node plus arity arena cells, except
    /// the two-node empty conjunction. Bounds cover total nodes and both total
    /// logical occurrences and raw arena length, not just this append's delta.
    /// The caller's borrowed input row is additional storage.
    ///
    /// # Errors
    /// Refuses an invalid child, exhausted inclusive dimension, checked overflow
    /// or failed reservation. Prior transaction appends remain unchanged on this
    /// call's refusal; dropping the transaction rolls those back as well.
    pub fn push(
        &mut self,
        node: NodeView<'_>,
        max_nodes: usize,
        max_operands: usize,
    ) -> Result<usize, AdmissionError> {
        let index = self.parts.nodes.len();
        let checked_prefix = *self.validated == index;
        validate_children(index, node)?;
        let (added_nodes, added_arena, added_edges) = match node {
            NodeView::And([]) => (2, 0, 2),
            NodeView::And([_]) | NodeView::Or([_]) => (0, 0, 0),
            NodeView::And(row) | NodeView::Or(row) => {
                (1, if row.len() >= 3 { row.len() } else { 0 }, row.len())
            }
            NodeView::Implies(_, _) => (1, 0, 2),
            NodeView::Atom(_) | NodeView::False => (1, 0, 0),
        };
        let node_count = index
            .checked_add(added_nodes)
            .ok_or(AdmissionError::Limit)?;
        let arena_count = self
            .parts
            .operands
            .len()
            .checked_add(added_arena)
            .ok_or(AdmissionError::Limit)?;
        let occurrences = self
            .parts
            .occurrences
            .checked_add(added_edges)
            .ok_or(AdmissionError::Limit)?;
        if node_count > max_nodes || arena_count > max_operands || occurrences > max_operands {
            return Err(AdmissionError::Limit);
        }
        self.parts
            .nodes
            .try_reserve(added_nodes)
            .map_err(|_| AdmissionError::Allocation)?;
        self.parts
            .operands
            .try_reserve(added_arena)
            .map_err(|_| AdmissionError::Allocation)?;
        let stored = match node {
            NodeView::Atom(atom) => Node::atom(atom),
            NodeView::False | NodeView::Or([]) => Node::falsum(),
            NodeView::Implies(left, right) => Node::implies(left, right),
            NodeView::And([]) => {
                self.parts.nodes.push(Node::falsum());
                self.parts.nodes.push(Node::implies(index, index));
                self.parts.occurrences = occurrences;
                if checked_prefix {
                    *self.validated = node_count;
                }
                return Ok(index + 1);
            }
            NodeView::And([child]) | NodeView::Or([child]) => return Ok(*child),
            NodeView::And([left, right]) => Node::and_pair([*left, *right]),
            NodeView::Or([left, right]) => Node::or_pair([*left, *right]),
            NodeView::And(row) | NodeView::Or(row) => {
                let span = OperandSpan {
                    start: self.parts.operands.len(),
                    length: row.len(),
                };
                self.parts.operands.extend_from_slice(row);
                if matches!(node, NodeView::And(_)) {
                    Node::and_span(span)
                } else {
                    Node::or_span(span)
                }
            }
        };
        self.parts.nodes.push(stored);
        self.parts.occurrences = occurrences;
        if checked_prefix {
            *self.validated = node_count;
        }
        Ok(index)
    }

    /// Begin a nested append, borrowing this transaction until it completes.
    /// Its rollback restores only its own suffix; the outer checkpoint is retained.
    #[must_use]
    pub fn transaction(&mut self) -> FormulaTransaction<'_> {
        FormulaTransaction::new(self.parts, self.validated)
    }

    /// Keep all appends and release the exclusive borrow; constant time.
    pub fn commit(mut self) {
        self.completed = true;
    }

    /// Detach this transaction's appended paired suffix and restore its prefix.
    ///
    /// Takes O(new nodes + new arena cells) work and temporary storage, without
    /// scanning or copying the retained prefix. Both destination reservations
    /// precede mutation. The old capacities remain retained after detachment, so
    /// enclosing peak accounting must include those plus the returned capacities.
    /// Child IDs remain absolute; only wide-span offsets are rebased to zero.
    ///
    /// # Errors
    /// Refuses failed reservation or an inconsistent internal span. On refusal
    /// this consumed transaction rolls back both buffers without publishing a
    /// suffix. The rollback retains actual capacity growth, as usual.
    pub fn detach(mut self) -> Result<FormulaSuffix, AdmissionError> {
        let mut nodes = Vec::new();
        let mut operands = Vec::new();
        nodes
            .try_reserve_exact(self.parts.nodes.len() - self.first)
            .map_err(|_| AdmissionError::Allocation)?;
        operands
            .try_reserve_exact(self.parts.operands.len() - self.operand_start)
            .map_err(|_| AdmissionError::Allocation)?;
        for node in &self.parts.nodes[self.first..] {
            nodes.push(node.rebase(self.operand_start)?);
        }
        operands.extend_from_slice(&self.parts.operands[self.operand_start..]);
        let occurrences = self.parts.occurrences - self.occurrences;
        self.restore();
        self.completed = true;
        Ok(FormulaSuffix {
            first: self.first,
            parts: FormulaParts {
                nodes,
                operands,
                occurrences,
            },
        })
    }

    pub(crate) fn validation_frontier(&self) -> usize {
        *self.validated
    }

    /// Completed scans may publish only a prefix of the current immutable nodes.
    pub(crate) fn set_validation_frontier(&mut self, frontier: usize) {
        *self.validated = frontier.min(self.parts.nodes.len());
    }

    fn restore(&mut self) {
        self.parts.nodes.truncate(self.first);
        self.parts.operands.truncate(self.operand_start);
        self.parts.occurrences = self.occurrences;
        *self.validated = self.initial_validated;
    }
}
impl Drop for FormulaTransaction<'_> {
    fn drop(&mut self) {
        if !self.completed {
            self.restore();
        }
    }
}

#[derive(Debug)]
struct Data {
    atoms: usize,
    parts: FormulaParts,
    roots: Vec<usize>,
}

/// One consuming admission attempt with a fixed validation route.
///
/// [`crate::FormulaNodes::prepare_admission`] preserves completed topology
/// checks without accepting a caller assertion. Raw or incompletely checked
/// owners use the full raw admission route. Dimensions, logical occurrences,
/// atom IDs and roots are checked independently on both routes. Preparation
/// transfers the existing buffers without copying, allocation or validation.
#[derive(Debug)]
pub struct TheoryAdmission {
    atoms: usize,
    parts: FormulaParts,
    roots: Vec<usize>,
    limits: AdmissionLimits,
    validation: AdmissionValidation,
}

#[derive(Debug)]
enum AdmissionValidation {
    Full,
    Atoms,
}

impl TheoryAdmission {
    pub(crate) fn new(
        atoms: usize,
        parts: FormulaParts,
        roots: Vec<usize>,
        limits: AdmissionLimits,
        validated: usize,
    ) -> Self {
        let validation = if validated == parts.nodes.len() {
            AdmissionValidation::Atoms
        } else {
            AdmissionValidation::Full
        };
        Self {
            atoms,
            parts,
            roots,
            limits,
            validation,
        }
    }

    /// Documented full remaining scan operations for this exact attempt.
    ///
    /// Both routes recount N nodes, inspect N nodes for atom IDs, and inspect R
    /// roots. The raw route additionally visits every logical child occurrence
    /// E while validating spans and topology: 2N + E + R rather than 2N + R.
    /// This excludes fixed dimension checks and handle publication. It is a
    /// pre-admission charge, not hardware instructions or actual work after an
    /// early refusal. No work budget or cancellation callback runs here.
    /// The u128 sum accommodates these usize populations on supported targets.
    #[must_use]
    pub fn work(&self) -> u128 {
        let edges = match self.validation {
            AdmissionValidation::Full => self.parts.occurrences as u128,
            AdmissionValidation::Atoms => 0,
        };
        2 * self.parts.nodes.len() as u128 + edges + self.roots.len() as u128
    }

    /// Complete the fixed admission route and transfer the unchanged buffers.
    ///
    /// # Errors
    /// Preserves raw admission's dimension/atom/root refusals. An unchecked
    /// owner also preserves its raw span/arity/edge errors and their order.
    /// Checked topology supplies only the omitted span and backward-edge checks;
    /// it supplies no atom-universe, root, bound or logical-truth premise.
    pub fn admit(self) -> Result<Theory, AdmissionError> {
        let Self {
            atoms,
            parts,
            roots,
            limits,
            validation,
        } = self;
        let data = match validation {
            AdmissionValidation::Full => admit(atoms, parts, roots, limits)?,
            AdmissionValidation::Atoms => admit_checked(atoms, parts, roots, limits)?,
        };
        Ok(Theory(Arc::new(data)))
    }
}

/// An immutable, instance-identified finite theory in topological order.
#[derive(Clone, Debug)]
pub struct Theory(Arc<Data>);
impl Theory {
    /// Admit a paired DAG in O(nodes + referenced operands + roots) time.
    ///
    /// Check dimensions first, independently counting logical occurrences from
    /// stored nodes rather than trusting the builder cache; then each node's
    /// canonical storage, complete span,
    /// atom and backward edges in stored order; then roots. Both vectors and the
    /// root vector transfer unchanged. Raw overlapping spans are accepted, with
    /// every occurrence counted; unused arena cells consume the arena ceiling.
    /// A refusal allocates nothing. Success performs the shared handle's one
    /// infallible `Arc` allocation; clones then share the handle in constant time.
    /// Caller-owned capacity is exposed through [`Self::parts`] for accounting.
    /// No evaluation, grounding, work-budget polling or search occurs.
    ///
    /// # Errors
    /// Refuses excessive dimensions, noncanonical or malformed spans, invalid
    /// atom/edge/root IDs, or a padded word count that cannot fit on this host.
    pub fn new(
        atoms: usize,
        parts: FormulaParts,
        roots: Vec<usize>,
        limits: AdmissionLimits,
    ) -> Result<Self, AdmissionError> {
        let data = admit(atoms, parts, roots, limits)?;
        Ok(Self(Arc::new(data)))
    }

    /// Number of atoms, whether or not they occur in an asserted formula.
    #[must_use]
    pub fn atom_count(&self) -> usize {
        self.0.atoms
    }

    /// The authoritative paired storage and its actual capacities.
    #[must_use]
    pub fn parts(&self) -> &FormulaParts {
        &self.0.parts
    }

    /// Storage-independent logical access to this admitted DAG.
    #[must_use]
    pub fn view(&self) -> FormulaView<'_> {
        self.0.parts.view()
    }

    /// Raw nodes for representation/transport inspection, paired with operands.
    #[must_use]
    pub fn nodes(&self) -> &[Node] {
        self.0.parts.nodes()
    }

    /// The shared wide-operand arena; inline pairs are in their own node cells.
    #[must_use]
    pub fn operands(&self) -> &[usize] {
        self.0.parts.operands()
    }

    /// Roots whose conjunction constitutes this theory.
    #[must_use]
    pub fn roots(&self) -> &[usize] {
        &self.0.roots
    }

    /// Clones share identity; independent equal admissions do not.
    #[must_use]
    pub fn same_instance(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

fn admit(
    atoms: usize,
    mut parts: FormulaParts,
    roots: Vec<usize>,
    limits: AdmissionLimits,
) -> Result<Data, AdmissionError> {
    let occurrences = admit_dimensions(atoms, &parts, &roots, limits)?;
    validate_nodes(atoms, parts.view())?;
    validate_roots(parts.nodes.len(), &roots)?;
    // Admission establishes the retained count even for an arbitrary extracted
    // FormulaParts record, without a premise that its private cache is correct.
    parts.occurrences = occurrences;
    Ok(Data {
        atoms,
        parts,
        roots,
    })
}

/// Both doors independently count actual logical occurrences. Checked topology
/// neither changes dimension precedence nor authorizes a cached-count shortcut.
fn admit_dimensions(
    atoms: usize,
    parts: &FormulaParts,
    roots: &[usize],
    limits: AdmissionLimits,
) -> Result<usize, AdmissionError> {
    if atoms > limits.max_atoms
        || parts.nodes.len() > limits.max_nodes
        || roots.len() > limits.max_roots
        || parts.operands.len() > limits.max_operands
        || atoms.checked_add(63).is_none()
    {
        return Err(AdmissionError::Limit);
    }
    let occurrences = count_occurrences(&parts.nodes)?;
    if occurrences > limits.max_operands {
        return Err(AdmissionError::Limit);
    }
    Ok(occurrences)
}

/// The consumed owner's completed frontier establishes canonical spans and
/// backward edges. Atom and root admission remain independent of that frontier.
fn admit_checked(
    atoms: usize,
    mut parts: FormulaParts,
    roots: Vec<usize>,
    limits: AdmissionLimits,
) -> Result<Data, AdmissionError> {
    let occurrences = admit_dimensions(atoms, &parts, &roots, limits)?;
    for (index, node) in parts.nodes.iter().enumerate() {
        if let Storage::Atom(atom) = node.0 {
            validate_node(atoms, index, NodeView::Atom(atom))?;
        }
    }
    validate_roots(parts.nodes.len(), &roots)?;
    parts.occurrences = occurrences;
    Ok(Data {
        atoms,
        parts,
        roots,
    })
}

/// Count declared occurrences without using a cached construction summary.
/// Malformed spans still contribute their declared length to dimension checks;
/// complete range and backward-edge checks follow in stored-node order.
fn count_occurrences(nodes: &[Node]) -> Result<usize, AdmissionError> {
    let mut occurrences = 0usize;
    for node in nodes {
        occurrences = occurrences
            .checked_add(node.occurrences())
            .ok_or(AdmissionError::Limit)?;
    }
    Ok(occurrences)
}

fn read_span(operands: &[usize], span: OperandSpan) -> Result<&[usize], AdmissionError> {
    if span.length < 3 {
        return Err(AdmissionError::Arity);
    }
    let end = span
        .start
        .checked_add(span.length)
        .ok_or(AdmissionError::Span)?;
    operands.get(span.start..end).ok_or(AdmissionError::Span)
}

/// Check one logical node against its atom universe and preceding-node prefix.
/// Resolving raw storage belongs to the caller; each complete child row is
/// checked here without assuming that the construction cache is correct.
fn validate_node(atoms: usize, index: usize, node: NodeView<'_>) -> Result<(), AdmissionError> {
    if let NodeView::Atom(atom) = node
        && atom >= atoms
    {
        return Err(AdmissionError::Atom);
    }
    validate_children(index, node)
}

/// Validate stored nodes in order, preserving the first refusal. Before each
/// iteration all earlier nodes have canonical storage and valid references.
fn validate_nodes(atoms: usize, view: FormulaView<'_>) -> Result<(), AdmissionError> {
    for index in 0..view.len() {
        validate_node(atoms, index, view.node(index)?)?;
    }
    Ok(())
}

fn validate_children(index: usize, node: NodeView<'_>) -> Result<(), AdmissionError> {
    match node {
        NodeView::Implies(left, right) if left >= index || right >= index => {
            Err(AdmissionError::Edge)
        }
        NodeView::And(row) | NodeView::Or(row) => {
            for &child in row {
                if child >= index {
                    return Err(AdmissionError::Edge);
                }
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn validate_root(node_count: usize, root: usize) -> Result<(), AdmissionError> {
    if root >= node_count {
        Err(AdmissionError::Root)
    } else {
        Ok(())
    }
}

fn validate_roots(node_count: usize, roots: &[usize]) -> Result<(), AdmissionError> {
    for &root in roots {
        validate_root(node_count, root)?;
    }
    Ok(())
}

/// Insert atoms into the fixed-length packed storage prepared by the constructor.
/// The words cover the declared atom universe. Each atom is checked before its
/// write; the first invalid atom leaves the remaining iterator unconsumed.
/// Earlier writes remain local to the constructor, which publishes only success.
fn insert_atoms(
    atom_count: usize,
    atoms: impl Iterator<Item = usize>,
    words: &mut [u64],
) -> Result<(), AdmissionError> {
    for atom in atoms {
        if atom >= atom_count {
            return Err(AdmissionError::Atom);
        }
        words[atom / 64] |= 1 << (atom % 64);
    }
    Ok(())
}

/// Packed membership in exactly one immutable theory's finite atom universe.
/// This is an arbitrary truth assignment, with no satisfaction or stability
/// claim. Cloning copies the packed words and shares the theory handle.
#[derive(Clone, Debug)]
pub struct Interpretation {
    pub(crate) theory: Theory,
    pub(crate) words: Vec<u64>,
}
impl Interpretation {
    /// Construct an interpretation, coalescing repeated atom indices.
    /// For a universe of U atoms and n input indices, this initializes
    /// ceil(U/64) words and consumes the iterator in O(ceil(U/64) + n) time,
    /// using O(ceil(U/64)) owned words. The iterator must terminate; its own cost
    /// is additional. No formula is evaluated. The theory handle is shared.
    ///
    /// # Errors
    /// Refuses out-of-universe atoms or failed storage reservation.
    pub fn new(
        theory: &Theory,
        atoms: impl IntoIterator<Item = usize>,
    ) -> Result<Self, AdmissionError> {
        let count = theory.atom_count().div_ceil(64);
        let mut words = Vec::new();
        words
            .try_reserve_exact(count)
            .map_err(|_| AdmissionError::Allocation)?;
        words.resize(count, 0);
        let iterator = atoms.into_iter();
        insert_atoms(theory.atom_count(), iterator, &mut words)?;
        Ok(Self {
            theory: theory.clone(),
            words,
        })
    }

    /// Copy the packed interpretation while retaining the exact immutable theory.
    ///
    /// This fallible clone initializes and copies ceil(U/64) words for U atoms,
    /// using O(ceil(U/64)) work and owned storage. It evaluates no formula and
    /// does not poll cancellation or charge an enclosing operation's budget;
    /// callers provide those controls around the bounded copy. The source is
    /// unchanged, including when allocation fails.
    ///
    /// # Errors
    /// Refuses failed storage reservation without publishing a partial copy.
    pub fn try_clone(&self) -> Result<Self, AdmissionError> {
        let mut words = Vec::new();
        words
            .try_reserve_exact(self.words.len())
            .map_err(|_| AdmissionError::Allocation)?;
        words.extend_from_slice(&self.words);
        Ok(Self {
            theory: self.theory.clone(),
            words,
        })
    }

    /// The theory instance to which this interpretation belongs.
    #[must_use]
    pub fn theory(&self) -> &Theory {
        &self.theory
    }

    /// Constant-time membership; an out-of-universe index is false.
    #[must_use]
    pub fn contains(&self, atom: usize) -> bool {
        atom < self.theory.atom_count() && self.words[atom / 64] & (1 << (atom % 64)) != 0
    }

    /// Atom indices in ascending order, without materializing a second carrier.
    /// Construction is constant time; complete traversal visits ceil(U/64)
    /// packed words and S selected atoms in O(ceil(U/64) + S) time, with constant
    /// auxiliary space. Each nonzero word loses its least set bit at each step.
    pub fn atoms(&self) -> impl Iterator<Item = usize> + '_ {
        self.words.iter().enumerate().flat_map(|(index, &word)| {
            let mut remaining = word;
            std::iter::from_fn(move || {
                if remaining == 0 {
                    return None;
                }
                let bit = remaining.trailing_zeros() as usize;
                remaining &= remaining - 1;
                Some(index * 64 + bit)
            })
        })
    }

    /// Borrow membership as ascending low-bit-first 32-bit words.
    ///
    /// The iterator retains this interpretation and its exact theory owner;
    /// equal dimensions never substitute for that identity. It exports exactly
    /// ceil(U/32) words, including zero words, with zero unused tail bits. No
    /// padding word is exported for an empty universe. Each step extracts one
    /// numeric half of a stored word, independently of host byte order.
    /// Construction and each step take constant time and allocate no storage.
    #[must_use]
    pub fn words32(&self) -> InterpretationWords<'_> {
        InterpretationWords {
            interpretation: self,
            next: 0,
            end: self.theory.atom_count().div_ceil(32),
        }
    }
}

/// Borrowed 32-bit membership words belonging to one exact interpretation.
///
/// Produced by [`Interpretation::words32`]. Advancing or cloning this iterator
/// changes only its cursor; membership and theory identity remain borrowed.
#[derive(Clone)]
pub struct InterpretationWords<'a> {
    interpretation: &'a Interpretation,
    next: usize,
    end: usize,
}

impl InterpretationWords<'_> {
    /// Exact theory whose atom positions these words encode; constant time.
    #[must_use]
    pub fn theory(&self) -> &Theory {
        self.interpretation.theory()
    }
}

impl Iterator for InterpretationWords<'_> {
    type Item = u32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.next == self.end {
            return None;
        }
        let word = self.interpretation.words[self.next / 2];
        let bytes = (word >> ((self.next % 2) * 32)).to_le_bytes();
        let half = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        self.next += 1;
        Some(half)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.len();
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for InterpretationWords<'_> {
    fn len(&self) -> usize {
        self.end - self.next
    }
}

impl std::iter::FusedIterator for InterpretationWords<'_> {}

#[cfg(test)]
mod tests {
    use super::*;

    fn parts(nodes: Vec<Node>, operands: Vec<usize>) -> FormulaParts {
        FormulaParts::new(nodes, operands).unwrap()
    }

    fn limits(max_operands: usize) -> AdmissionLimits {
        AdmissionLimits {
            max_operands,
            ..AdmissionLimits::default()
        }
    }

    fn wide(span: OperandSpan) -> FormulaParts {
        parts(vec![Node::falsum(), Node::and_span(span)], vec![0, 0, 0])
    }

    #[test]
    fn checked_admission_recounts_actual_occurrences() {
        for cached in [0, usize::MAX] {
            for cap in [2, 3] {
                let mut graph = wide(OperandSpan {
                    start: 0,
                    length: 3,
                });
                // The paired graph has checked topology. Corrupt only the cache
                // to discriminate an independent recount from trusting it.
                graph.occurrences = cached;
                let admission = TheoryAdmission::new(0, graph, vec![1], limits(cap), 2);
                if cap == 2 {
                    assert_eq!(admission.admit().unwrap_err(), AdmissionError::Limit);
                } else {
                    assert_eq!(admission.admit().unwrap().parts().occurrences(), 3);
                }
            }
        }
    }

    #[test]
    fn flat_storage_keeps_three_native_words() {
        assert_eq!(size_of::<Node>(), 3 * size_of::<usize>());
        assert_eq!(align_of::<Node>(), align_of::<usize>());
        assert_eq!(size_of::<Node>(), size_of::<Storage>());
        assert_eq!(align_of::<Node>(), align_of::<Storage>());
        assert_eq!(size_of::<OperandSpan>(), 2 * size_of::<usize>());
        assert_eq!(align_of::<OperandSpan>(), align_of::<usize>());
    }

    #[test]
    fn admission_keeps_all_supplied_buffers() {
        let mut nodes = Vec::with_capacity(8);
        nodes.extend([
            Node::falsum(),
            Node::atom(0),
            Node::and_pair([0, 1]),
            Node::or_span(OperandSpan {
                start: 0,
                length: 3,
            }),
        ]);
        let mut operands = Vec::with_capacity(8);
        operands.extend([0, 1, 2]);
        let mut roots = Vec::with_capacity(8);
        roots.extend([3, 3, 0]);
        let node_buffer = (nodes.as_ptr(), nodes.capacity());
        let operand_buffer = (operands.as_ptr(), operands.capacity());
        let root_buffer = (roots.as_ptr(), roots.capacity());
        let theory = Theory::new(1, parts(nodes, operands), roots, limits(5)).unwrap();
        assert_eq!(
            (theory.nodes().as_ptr(), theory.parts().node_capacity()),
            node_buffer
        );
        assert_eq!(
            (
                theory.operands().as_ptr(),
                theory.parts().operand_capacity()
            ),
            operand_buffer
        );
        assert_eq!(
            (theory.0.roots.as_ptr(), theory.0.roots.capacity()),
            root_buffer
        );
    }

    #[test]
    fn raw_spans_require_canonical_wide_arity() {
        for length in 0..3 {
            let error = Theory::new(
                0,
                wide(OperandSpan { start: 0, length }),
                vec![1],
                limits(3),
            )
            .unwrap_err();
            assert_eq!(error, AdmissionError::Arity);
        }
        assert!(
            Theory::new(
                0,
                wide(OperandSpan {
                    start: 0,
                    length: 3
                }),
                vec![1],
                limits(3)
            )
            .is_ok()
        );
    }

    #[test]
    fn raw_spans_require_the_complete_range() {
        for start in [1, usize::MAX] {
            let error = Theory::new(
                0,
                wide(OperandSpan { start, length: 3 }),
                vec![1],
                limits(3),
            )
            .unwrap_err();
            assert_eq!(error, AdmissionError::Span);
        }
    }

    #[test]
    fn raw_wide_edges_must_all_point_backward() {
        let graph = parts(
            vec![
                Node::falsum(),
                Node::or_span(OperandSpan {
                    start: 0,
                    length: 3,
                }),
            ],
            vec![0, 1, 0],
        );
        assert_eq!(
            Theory::new(0, graph, vec![1], limits(3)).unwrap_err(),
            AdmissionError::Edge
        );
    }

    #[test]
    fn overlapping_spans_count_each_occurrence() {
        let graph = || {
            parts(
                vec![
                    Node::falsum(),
                    Node::and_span(OperandSpan {
                        start: 0,
                        length: 3,
                    }),
                    Node::or_span(OperandSpan {
                        start: 0,
                        length: 3,
                    }),
                ],
                vec![0, 0, 0],
            )
        };
        let theory = Theory::new(0, graph(), vec![2], limits(6)).unwrap();
        assert_eq!(theory.parts().occurrences(), 6);
        assert_eq!(theory.view().node(1).unwrap(), NodeView::And(&[0, 0, 0]));
        assert_eq!(theory.view().node(2).unwrap(), NodeView::Or(&[0, 0, 0]));
        assert_eq!(
            Theory::new(0, graph(), vec![2], limits(5)).unwrap_err(),
            AdmissionError::Limit
        );
    }

    #[test]
    fn admission_does_not_trust_an_undercounted_cache() {
        // Private-field corruption models an arbitrary extracted record. Public
        // Rust construction cannot create it, but admission needs no such premise.
        let mut graph = parts(vec![Node::falsum(), Node::and_pair([0, 0])], vec![]);
        graph.occurrences = 0;
        assert_eq!(
            Theory::new(0, graph, vec![1], limits(1)).unwrap_err(),
            AdmissionError::Limit
        );
    }

    #[test]
    fn admission_establishes_the_retained_occurrence_count() {
        let mut graph = wide(OperandSpan {
            start: 0,
            length: 3,
        });
        graph.occurrences = usize::MAX;
        let theory = Theory::new(0, graph, vec![1], limits(3)).unwrap();
        assert_eq!(theory.parts().occurrences(), 3);
    }

    #[test]
    fn inline_and_implication_edges_consume_the_limit() {
        let graph = || {
            parts(
                vec![Node::falsum(), Node::implies(0, 0), Node::and_pair([0, 1])],
                vec![],
            )
        };
        assert!(Theory::new(0, graph(), vec![2], limits(4)).is_ok());
        assert_eq!(
            Theory::new(0, graph(), vec![2], limits(3)).unwrap_err(),
            AdmissionError::Limit
        );
    }

    #[test]
    fn unused_raw_arena_cells_consume_the_limit() {
        let graph = || parts(vec![Node::falsum()], vec![usize::MAX; 3]);
        assert!(Theory::new(0, graph(), vec![0], limits(3)).is_ok());
        assert_eq!(
            Theory::new(0, graph(), vec![0], limits(2)).unwrap_err(),
            AdmissionError::Limit
        );
    }

    #[test]
    fn declared_occurrence_overflow_is_typed() {
        assert_eq!(
            FormulaParts::new(
                vec![
                    Node::and_span(OperandSpan {
                        start: 0,
                        length: usize::MAX
                    }),
                    Node::implies(0, 0)
                ],
                vec![]
            )
            .unwrap_err(),
            AdmissionError::Limit
        );
    }

    #[test]
    fn dimension_refusal_precedes_malformed_storage() {
        assert_eq!(
            Theory::new(
                0,
                wide(OperandSpan {
                    start: usize::MAX,
                    length: 3
                }),
                vec![9],
                limits(2)
            )
            .unwrap_err(),
            AdmissionError::Limit
        );
    }

    #[test]
    fn stored_node_order_precedes_later_shape_errors() {
        let graph = parts(
            vec![
                Node::atom(1),
                Node::and_span(OperandSpan {
                    start: usize::MAX,
                    length: 3,
                }),
            ],
            vec![],
        );
        assert_eq!(
            Theory::new(0, graph, vec![9], limits(3)).unwrap_err(),
            AdmissionError::Atom
        );
    }

    #[test]
    fn storage_refusal_precedes_root_refusal() {
        assert_eq!(
            Theory::new(
                0,
                wide(OperandSpan {
                    start: 1,
                    length: 3
                }),
                vec![9],
                limits(3)
            )
            .unwrap_err(),
            AdmissionError::Span
        );
    }

    #[test]
    fn owned_groups_use_one_canonical_physical_form() {
        let mut graph = parts(vec![Node::falsum()], vec![]);
        let mut validated = 0;
        let mut transaction = FormulaTransaction::new(&mut graph, &mut validated);
        assert_eq!(transaction.push(NodeView::And(&[0, 0]), 3, 5).unwrap(), 1);
        assert_eq!(transaction.push(NodeView::Or(&[1, 0, 1]), 3, 5).unwrap(), 2);
        transaction.commit();
        assert_eq!(
            graph.nodes(),
            &[
                Node::falsum(),
                Node::and_pair([0, 0]),
                Node::or_span(OperandSpan {
                    start: 0,
                    length: 3
                })
            ]
        );
        assert_eq!(graph.operands(), &[1, 0, 1]);
        assert_eq!(graph.occurrences(), 5);
    }

    #[test]
    fn owned_empty_and_singleton_groups_are_normalized() {
        let mut graph = FormulaParts::default();
        let mut validated = 0;
        let mut transaction = FormulaTransaction::new(&mut graph, &mut validated);
        assert_eq!(transaction.push(NodeView::Or(&[]), 3, 2).unwrap(), 0);
        assert_eq!(transaction.push(NodeView::And(&[0]), 3, 2).unwrap(), 0);
        assert_eq!(transaction.push(NodeView::Or(&[0]), 3, 2).unwrap(), 0);
        assert_eq!(transaction.push(NodeView::And(&[]), 3, 2).unwrap(), 2);
        transaction.commit();
        assert_eq!(
            graph.nodes(),
            &[Node::falsum(), Node::falsum(), Node::implies(1, 1)]
        );
        assert!(graph.operands().is_empty());
    }

    #[test]
    fn refused_group_preserves_both_lengths() {
        let mut graph = parts(vec![Node::falsum()], vec![]);
        let mut validated = 0;
        let mut transaction = FormulaTransaction::new(&mut graph, &mut validated);
        assert_eq!(
            transaction
                .push(NodeView::And(&[0, 0, 0]), 2, 2)
                .unwrap_err(),
            AdmissionError::Limit
        );
        assert_eq!(transaction.parts().nodes(), &[Node::falsum()]);
        assert!(transaction.parts().operands().is_empty());
        assert_eq!(transaction.parts().occurrences(), 0);
        transaction.commit();
    }

    #[test]
    fn owned_wide_limit_is_inclusive() {
        let mut graph = parts(vec![Node::falsum()], vec![]);
        let mut validated = 0;
        let mut transaction = FormulaTransaction::new(&mut graph, &mut validated);
        assert_eq!(
            transaction.push(NodeView::And(&[0, 0, 0]), 2, 3).unwrap(),
            1
        );
        transaction.commit();
        assert!(Theory::new(0, graph, vec![1], limits(3)).is_ok());
    }

    #[test]
    fn rollback_restores_the_paired_prefix_and_frontier() {
        let mut graph = wide(OperandSpan {
            start: 0,
            length: 3,
        });
        let mut validated = 1;
        {
            let mut transaction = FormulaTransaction::new(&mut graph, &mut validated);
            transaction.push(NodeView::Or(&[0, 1, 0]), 3, 6).unwrap();
            transaction.set_validation_frontier(3);
        }
        assert_eq!(graph.nodes().len(), 2);
        assert_eq!(graph.operands(), &[0, 0, 0]);
        assert_eq!(graph.occurrences(), 3);
        // Only validation established at the checkpoint survives rollback.
        assert_eq!(validated, 1);
    }

    #[test]
    fn detached_suffix_keeps_children_and_rebases_spans() {
        let mut graph = wide(OperandSpan {
            start: 0,
            length: 3,
        });
        let mut validated = 2;
        let mut transaction = FormulaTransaction::new(&mut graph, &mut validated);
        transaction.push(NodeView::And(&[0, 1, 0]), 4, 8).unwrap();
        transaction.push(NodeView::Or(&[2, 1]), 4, 8).unwrap();
        let suffix = transaction.detach().unwrap();
        assert_eq!(suffix.first(), 2);
        assert_eq!(
            suffix.parts().nodes(),
            &[
                Node::and_span(OperandSpan {
                    start: 0,
                    length: 3
                }),
                Node::or_pair([2, 1])
            ]
        );
        assert_eq!(suffix.view().node(0).unwrap(), NodeView::And(&[0, 1, 0]));
        assert_eq!(suffix.view().node(1).unwrap(), NodeView::Or(&[2, 1]));
        assert_eq!(graph.operands(), &[0, 0, 0]);
        assert_eq!(graph.nodes().len(), 2);
        // Mutating the destination after detachment cannot overwrite suffix rows.
        let mut replacement = FormulaTransaction::new(&mut graph, &mut validated);
        replacement.push(NodeView::Or(&[1, 1, 1]), 3, 6).unwrap();
        replacement.commit();
        assert_eq!(suffix.view().node(0).unwrap(), NodeView::And(&[0, 1, 0]));
    }

    #[test]
    fn nested_rollback_retains_earlier_outer_appends() {
        let mut graph = parts(vec![Node::falsum()], vec![]);
        let mut validated = 0;
        let mut outer = FormulaTransaction::new(&mut graph, &mut validated);
        outer.push(NodeView::And(&[0, 0, 0]), 3, 6).unwrap();
        {
            let mut inner = outer.transaction();
            inner.push(NodeView::Or(&[0, 1, 0]), 3, 6).unwrap();
        }
        assert_eq!(outer.view().len(), 2);
        assert_eq!(outer.parts().operands(), &[0, 0, 0]);
        outer.commit();
        assert_eq!(graph.occurrences(), 3);
    }

    fn incomplete_raw_prefix() -> FormulaParts {
        parts(
            vec![
                Node::falsum(),
                Node::or_span(OperandSpan {
                    start: 0,
                    length: 3,
                }),
            ],
            vec![],
        )
    }

    fn temporarily_validate_prefix(transaction: &mut FormulaTransaction<'_>) {
        transaction.push(NodeView::And(&[0, 0, 0]), 3, 6).unwrap();
        transaction
            .append_aggregate_family(
                &[],
                &[],
                crate::AggregateFamilyLimits::default(),
                &zetesis_cpu::Cancellation::default(),
            )
            .unwrap();
        assert_eq!(transaction.validation_frontier(), 3);
    }

    fn rejects_restored_raw_prefix(parts: &mut FormulaParts, validated: &mut usize) {
        assert_eq!(parts.view().node(1).unwrap_err(), AdmissionError::Span);
        let mut transaction = FormulaTransaction::new(parts, validated);
        let error = transaction
            .append_aggregate_family(
                &[],
                &[],
                crate::AggregateFamilyLimits::default(),
                &zetesis_cpu::Cancellation::default(),
            )
            .unwrap_err();
        assert_eq!(
            error.kind,
            crate::AggregateErrorKind::InvalidPrefix { node: 1 }
        );
        drop(transaction);
        assert_eq!(*validated, 0);
    }

    #[test]
    fn drop_discards_validation_that_borrowed_appended_cells() {
        let mut parts = incomplete_raw_prefix();
        let mut validated = 0;
        {
            let mut transaction = FormulaTransaction::new(&mut parts, &mut validated);
            temporarily_validate_prefix(&mut transaction);
        }
        rejects_restored_raw_prefix(&mut parts, &mut validated);
    }

    #[test]
    fn detach_discards_validation_that_borrowed_appended_cells() {
        let mut parts = incomplete_raw_prefix();
        let mut validated = 0;
        let mut transaction = FormulaTransaction::new(&mut parts, &mut validated);
        temporarily_validate_prefix(&mut transaction);
        let suffix = transaction.detach().unwrap();
        assert_eq!(suffix.view().node(0).unwrap(), NodeView::And(&[0, 0, 0]));
        rejects_restored_raw_prefix(&mut parts, &mut validated);
    }

    #[test]
    fn nested_rollback_discards_validation_that_borrowed_its_cells() {
        let mut parts = incomplete_raw_prefix();
        let mut validated = 0;
        let mut outer = FormulaTransaction::new(&mut parts, &mut validated);
        {
            let mut inner = outer.transaction();
            temporarily_validate_prefix(&mut inner);
        }
        assert_eq!(outer.validation_frontier(), 0);
        assert_eq!(outer.view().node(1).unwrap_err(), AdmissionError::Span);
        outer.commit();
        rejects_restored_raw_prefix(&mut parts, &mut validated);
    }

    #[test]
    fn checked_appends_extend_the_complete_topology_prefix() {
        let mut graph = FormulaParts::default();
        let mut validated = 0;
        let mut transaction = FormulaTransaction::new(&mut graph, &mut validated);
        for node in [
            NodeView::False,
            NodeView::Atom(0),
            NodeView::Implies(0, 1),
            NodeView::And(&[0, 1]),
            NodeView::Or(&[1, 0]),
            NodeView::And(&[0, 1, 2]),
            NodeView::Or(&[0, 2, 1]),
            NodeView::And(&[]),
            NodeView::Or(&[]),
            NodeView::And(&[0]),
            NodeView::Or(&[0]),
        ] {
            transaction.push(node, 16, 32).unwrap();
            assert_eq!(transaction.validation_frontier(), transaction.view().len());
        }
        transaction.commit();
        assert_eq!(validated, graph.nodes().len());
    }

    #[test]
    fn checked_appends_do_not_skip_an_unchecked_raw_prefix() {
        let mut graph = parts(vec![Node::falsum(), Node::and_pair([0, 2])], vec![]);
        let mut validated = 1;
        let mut transaction = FormulaTransaction::new(&mut graph, &mut validated);
        transaction.push(NodeView::And(&[0, 0, 0]), 3, 5).unwrap();
        assert_eq!(transaction.validation_frontier(), 1);
        let error = transaction
            .append_aggregate_family(
                &[],
                &[],
                crate::AggregateFamilyLimits::default(),
                &zetesis_cpu::Cancellation::default(),
            )
            .unwrap_err();
        assert_eq!(
            error.kind,
            crate::AggregateErrorKind::InvalidPrefix { node: 1 }
        );
    }

    #[test]
    fn refused_appends_do_not_advance_the_topology_frontier() {
        let mut graph = parts(vec![Node::falsum()], vec![]);
        let mut validated = 1;
        let mut transaction = FormulaTransaction::new(&mut graph, &mut validated);
        assert_eq!(
            transaction
                .push(NodeView::And(&[0, 0, 1]), 2, 3)
                .unwrap_err(),
            AdmissionError::Edge
        );
        assert_eq!(
            transaction
                .push(NodeView::And(&[0, 0, 0]), 2, 2)
                .unwrap_err(),
            AdmissionError::Limit
        );
        assert_eq!(transaction.validation_frontier(), 1);
        assert_eq!(transaction.view().len(), 1);
        assert!(transaction.parts().operands().is_empty());
    }

    #[test]
    fn checked_topology_does_not_admit_the_atom_universe() {
        let mut graph = FormulaParts::default();
        let mut validated = 0;
        let mut transaction = FormulaTransaction::new(&mut graph, &mut validated);
        transaction.push(NodeView::Atom(0), 1, 0).unwrap();
        assert_eq!(transaction.validation_frontier(), 1);
        transaction.commit();
        assert_eq!(
            Theory::new(0, graph, vec![], AdmissionLimits::default()).unwrap_err(),
            AdmissionError::Atom
        );
    }

    #[test]
    fn checked_prefix_reuse_survives_detached_compilation() {
        let mut nodes = crate::FormulaNodes::default();
        let mut transaction = nodes.transaction();
        transaction.push(NodeView::Atom(0), 64, 128).unwrap();
        transaction.commit();
        let mut allowance = None;
        for _ in 0..4 {
            let previous = nodes.view().len() - 1;
            let mut source = nodes.transaction();
            let condition = source
                .push(NodeView::And(&[0, previous, 0]), 64, 128)
                .unwrap();
            source.commit();
            let mut compiled = nodes.transaction();
            let mut limits = crate::AggregateFamilyLimits::default();
            if let Some(work) = allowance {
                limits.aggregate.max_work = work;
            }
            let build = compiled
                .append_aggregate_family(
                    &[crate::AggregateElement {
                        weight: 1,
                        condition,
                    }],
                    &[crate::AggregateGuard {
                        comparison: crate::AggregateComparison::Ge,
                        bound: 1,
                    }],
                    limits,
                    &zetesis_cpu::Cancellation::default(),
                )
                .unwrap();
            if let Some(work) = allowance {
                assert_eq!(build.statistics().work, work);
            }
            allowance = Some(build.statistics().work);
            let suffix = compiled.detach().unwrap();
            assert_eq!(suffix.first(), condition + 1);
            assert_eq!(nodes.view().len(), condition + 1);
        }
    }
}
