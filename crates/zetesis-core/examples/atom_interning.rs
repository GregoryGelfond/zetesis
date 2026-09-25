//! Bounded appendable atom ownership: checked identities, rounds and capacities.
//!
//! `--check` omits timings. With no arguments, this fixed single-threaded library
//! probe also reports phase intervals; it does not measure a solver or RSS.

use std::{
    error::Error,
    io::{self, Write},
    mem::size_of,
    time::Instant,
};

use zetesis_core::{
    Atom, AtomPattern, Predicate, Sign, Term, Value, ValueLimits, ValueNode,
    atom_interner::{AtomInterner, Limits},
    catalog::AtomRef,
};

const SIZES: [usize; 3] = [128, 1_024, 4_096];
const ROUNDS: usize = 4;
const REPEATS: usize = 4;
const MISSES: usize = 16;
const WORK_LIMIT: u64 = 100_000_000;
// Explicit fixture allowance for short text, depth-three terms and four sealed
// rounds. Population alone does not bound arbitrary canonical term storage.
const CANONICAL_BYTES: usize = 64 * 1024 * 1024;

struct Fixture {
    atoms: Vec<Atom>,
    patterns: [AtomPattern; 2],
    misses: Vec<Atom>,
}

impl Fixture {
    fn new(rows: usize) -> Result<Self, Box<dyn Error>> {
        let patterns = [Sign::Positive, Sign::Negative].map(|sign| {
            AtomPattern::new(Predicate::with_sign("p", 1, sign)?, vec![Term::Variable(0)])
        });
        let [positive, negative] = patterns;
        let patterns = [positive?, negative?];
        // Paired rows have identical values and opposite predicate signs. Three
        // adjacent pairs share numeric contents but have different value types.
        let atoms = (0..rows)
            .rev()
            .map(|ordinal| {
                Ok(Atom::new(
                    patterns[ordinal % 2].predicate().clone(),
                    vec![value(ordinal / 2)?],
                )?)
            })
            .collect::<Result<Vec<_>, Box<dyn Error>>>()?;
        let misses = (0..MISSES)
            .map(|index| {
                Atom::new(
                    patterns[index % 2].predicate().clone(),
                    vec![Value::Symbol(format!("absent_{index}"))],
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        // Expected membership uses derived typed equality, independently of the
        // interner's checked comparison and AVL traversal.
        for missing in &misses {
            assert!(!atoms.contains(missing));
        }
        Ok(Self {
            atoms,
            patterns,
            misses,
        })
    }

    fn pattern(&self, atom: &Atom) -> &AtomPattern {
        match atom.predicate().sign() {
            Sign::Positive => &self.patterns[0],
            Sign::Negative => &self.patterns[1],
        }
    }

    fn ordered_ids(&self, committed: usize) -> Vec<usize> {
        let mut ids: Vec<_> = (0..committed).collect();
        ids.sort_unstable_by(|&a, &b| self.atoms[a].cmp(&self.atoms[b]));
        assert!(
            ids.windows(2)
                .all(|pair| self.atoms[pair[0]] < self.atoms[pair[1]])
        );
        ids
    }
}

fn value(group: usize) -> Result<Value, Box<dyn Error>> {
    let number = i32::try_from(group / 3)?;
    match group % 3 {
        0 => Ok(Value::Number(number)),
        1 => Ok(Value::String(number.to_string())),
        _ => Ok(Value::from_nodes(
            vec![
                ValueNode::Function {
                    name: "f_é".into(),
                    sign: Sign::Negative,
                    arity: 2,
                },
                ValueNode::Number(number),
                ValueNode::Tuple { arity: 1 },
                ValueNode::Number(number),
            ],
            ValueLimits::default(),
        )?),
    }
}

struct Phase {
    start: Option<Instant>,
    work: u64,
}

impl Phase {
    fn new(timed: bool) -> Self {
        Self {
            start: timed.then(Instant::now),
            work: 0,
        }
    }

    fn tick(&mut self) -> io::Result<()> {
        if self.work >= WORK_LIMIT {
            return Err(io::Error::other("probe work limit"));
        }
        self.work += 1;
        Ok(())
    }

    fn report(
        self,
        out: &mut impl Write,
        probe: &Probe,
        name: &str,
        counts: (usize, usize),
        order_bytes: u128,
    ) -> io::Result<()> {
        let nanos = self.start.map_or_else(
            || "not_measured".into(),
            |start| start.elapsed().as_nanos().to_string(),
        );
        let retained = probe.atoms.storage_bytes();
        let peak = probe.atoms.storage_peak_bytes();
        assert!(retained + order_bytes <= peak);
        assert!(peak <= probe.limits.max_bytes);
        writeln!(
            out,
            "{},{},{name},{},{},{},{},{},{nanos},{retained},{peak},{order_bytes},{},{WORK_LIMIT}",
            probe.limits.max_atoms,
            probe.round,
            probe.atoms.committed().len(),
            probe.atoms.len(),
            counts.0,
            counts.1,
            self.work,
            probe.limits.max_bytes,
        )
    }
}

#[derive(Clone, Copy)]
enum QueryDoor {
    Atom,
    Key,
}

struct Probe {
    atoms: AtomInterner,
    limits: Limits,
    round: usize,
    timed: bool,
}

impl Probe {
    fn append(&mut self, fixture: &Fixture, out: &mut impl Write) -> Result<(), Box<dyn Error>> {
        let start = self.atoms.len();
        let end = self.round * self.limits.max_atoms / ROUNDS;
        let mut phase = Phase::new(self.timed);
        {
            let (committed, mut appender) = self.atoms.split();
            for (id, atom) in fixture.atoms.iter().enumerate().take(end).skip(start) {
                let key = fixture.pattern(atom).key(atom.values())?;
                let entry = appender.entry_key_with(key, self.limits, || phase.tick())?;
                assert_eq!(entry.position(), None);
                assert_eq!(entry.insert_with(self.limits, || phase.tick())?, id);
                assert_eq!(appender.get(id), Some(AtomRef::from(atom)));
                assert_eq!(committed.get(id), None);
            }
            assert!(
                committed
                    .atoms()
                    .iter()
                    .eq(fixture.atoms[..start].iter().map(AtomRef::from))
            );
        }
        assert_eq!(self.atoms.len(), end);
        phase.report(out, self, "append_key", (end - start, 2 * (end - start)), 0)?;
        Ok(())
    }

    fn duplicates(
        &mut self,
        fixture: &Fixture,
        door: QueryDoor,
        out: &mut impl Write,
    ) -> Result<(), Box<dyn Error>> {
        let count = self.atoms.len();
        let mut phase = Phase::new(self.timed);
        for _ in 0..REPEATS {
            for (id, atom) in fixture.atoms.iter().enumerate().take(count) {
                let entry = match door {
                    QueryDoor::Atom => self
                        .atoms
                        .entry_atom_with(atom, self.limits, || phase.tick())?,
                    QueryDoor::Key => self.atoms.entry_key_with(
                        fixture.pattern(atom).key(atom.values())?,
                        self.limits,
                        || phase.tick(),
                    )?,
                };
                assert_eq!(entry.position(), Some(id));
                let before = phase.work;
                assert_eq!(entry.insert_with(self.limits, || phase.tick())?, id);
                assert_eq!(phase.work, before); // Occupied return has no copy/write work.
            }
        }
        assert_eq!(self.atoms.len(), count);
        let name = match door {
            QueryDoor::Atom => "duplicates_atom",
            QueryDoor::Key => "duplicates_key",
        };
        phase.report(out, self, name, (count * REPEATS, 2 * count * REPEATS), 0)?;
        Ok(())
    }

    fn misses(&mut self, fixture: &Fixture, out: &mut impl Write) -> Result<(), Box<dyn Error>> {
        let count = self.atoms.len();
        let mut phase = Phase::new(self.timed);
        for _ in 0..REPEATS {
            for atom in &fixture.misses {
                let key = fixture.pattern(atom).key(atom.values())?;
                let entry = self
                    .atoms
                    .entry_key_with(key, self.limits, || phase.tick())?;
                assert_eq!(entry.position(), None);
            }
        }
        assert_eq!(self.atoms.len(), count);
        phase.report(
            out,
            self,
            "misses_key",
            (MISSES * REPEATS, MISSES * REPEATS),
            0,
        )?;
        Ok(())
    }

    fn snapshot(
        &mut self,
        fixture: &Fixture,
        name: &str,
        out: &mut impl Write,
    ) -> Result<(), Box<dyn Error>> {
        let committed = self.atoms.committed().len();
        let expected = fixture.ordered_ids(committed);
        let mut phase = Phase::new(self.timed);
        let ids = self.atoms.ordered_ids_with(self.limits, || phase.tick())?;
        assert_eq!(ids, expected);
        for &id in &ids {
            assert_eq!(
                self.atoms.committed().get(id),
                Some(AtomRef::from(&fixture.atoms[id]))
            );
        }
        let order_bytes =
            size_of::<Vec<usize>>() as u128 + ids.capacity() as u128 * size_of::<usize>() as u128;
        phase.report(out, self, name, (ids.len(), 1), order_bytes)?;
        // The returned order is dropped here, before the next library operation.
        Ok(())
    }

    fn commit(&mut self, fixture: &Fixture, out: &mut impl Write) -> Result<(), Box<dyn Error>> {
        let added = self.atoms.len() - self.atoms.committed().len();
        let mut phase = Phase::new(self.timed);
        self.atoms.commit_with(self.limits, || phase.tick())?;
        assert!(
            self.atoms
                .committed()
                .atoms()
                .iter()
                .eq(fixture.atoms[..self.atoms.len()].iter().map(AtomRef::from))
        );
        phase.report(out, self, "commit", (added, 1), 0)?;
        Ok(())
    }
}

fn run(rows: usize, timed: bool, out: &mut impl Write) -> Result<(), Box<dyn Error>> {
    let fixture = Fixture::new(rows)?;
    let mut probe = Probe {
        atoms: AtomInterner::new(),
        limits: Limits::for_atoms(rows, CANONICAL_BYTES),
        round: 0,
        timed,
    };
    assert!(probe.atoms.is_empty());
    for round in 1..=ROUNDS {
        probe.round = round;
        probe.append(&fixture, out)?;
        probe.duplicates(&fixture, QueryDoor::Atom, out)?;
        probe.duplicates(&fixture, QueryDoor::Key, out)?;
        probe.misses(&fixture, out)?;
        probe.snapshot(&fixture, "snapshot_pending", out)?;
        probe.commit(&fixture, out)?;
        probe.snapshot(&fixture, "snapshot_committed", out)?;
    }
    let mut final_work = Phase::new(false);
    let atoms = probe
        .atoms
        .into_catalog_with(probe.limits, || final_work.tick())?;
    // Final publication meters the portable occurrence measure even though no
    // pending discovery remains. It transfers canonical references, not atoms.
    assert!(
        atoms
            .atoms()
            .iter()
            .eq(fixture.atoms.iter().map(AtomRef::from))
    );
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args().skip(1);
    let timed = match (args.next().as_deref(), args.next()) {
        (None, None) => true,
        (Some("--check"), None) => false,
        _ => return Err(io::Error::other("usage: atom_interning [--check]").into()),
    };
    let mut out = io::BufWriter::new(io::stdout().lock());
    writeln!(
        out,
        "target_rows,round,phase,committed_rows,total_rows,items,interner_calls,work,elapsed_ns,interner_retained_bytes,interner_peak_bytes,ordered_ids_bytes,byte_limit,work_limit"
    )?;
    for rows in SIZES {
        run(rows, timed, &mut out)?;
    }
    out.flush()?;
    Ok(())
}
