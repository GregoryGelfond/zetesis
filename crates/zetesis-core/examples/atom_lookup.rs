//! Bounded lookup scaling: one atom owner, exact reference rows, explicit setup.
//!
//! Run with `--check` for deterministic correctness/work/storage receipts only.
//! Running without arguments additionally times this fixed single-threaded probe;
//! it is neither an ordinary solver benchmark nor a process-memory measurement.

use std::{
    error::Error,
    io::{self, Write},
    mem::size_of,
    time::Instant,
};

use zetesis_core::{
    Atom, AtomCatalog, AtomIndex, AtomLookup, Model, Predicate, Sign, Value, ValueLimits, ValueNode,
};

const WORK_LIMIT: u64 = 100_000_000;
const QUERIES: usize = 16;
const REPEATS: usize = 8;

#[derive(Clone, Copy)]
struct Case {
    rows: usize,
    predicates: usize,
    depth: usize,
}

const CASES: [Case; 7] = [
    Case {
        rows: 128,
        predicates: 16,
        depth: 0,
    },
    Case {
        rows: 1_024,
        predicates: 16,
        depth: 0,
    },
    Case {
        rows: 4_096,
        predicates: 16,
        depth: 0,
    },
    Case {
        rows: 1_024,
        predicates: 1,
        depth: 0,
    },
    Case {
        rows: 1_024,
        predicates: 256,
        depth: 0,
    },
    Case {
        rows: 1_024,
        predicates: 16,
        depth: 8,
    },
    Case {
        rows: 1_024,
        predicates: 16,
        depth: 32,
    },
];

#[derive(Default)]
struct Work(u64);
impl Work {
    fn tick(&mut self) -> io::Result<()> {
        if self.0 >= WORK_LIMIT {
            return Err(io::Error::other("probe work limit"));
        }
        self.0 += 1;
        Ok(())
    }
}

struct Clock(Option<Instant>);
impl Clock {
    fn new(timed: bool) -> Self {
        Self(timed.then(Instant::now))
    }
    fn elapsed(self) -> String {
        self.0.map_or_else(
            || "not_measured".into(),
            |start| start.elapsed().as_nanos().to_string(),
        )
    }
}

fn value(index: usize, depth: usize) -> Result<Value, Box<dyn Error>> {
    let number = i32::try_from(index)?;
    if depth == 0 {
        return Ok(Value::Number(number));
    }
    let mut nodes: Vec<_> = (0..depth)
        .map(|_| ValueNode::Function {
            name: "shared_é_prefix".into(),
            sign: Sign::Negative,
            arity: 1,
        })
        .collect();
    nodes.push(ValueNode::Number(number));
    Ok(Value::from_nodes(nodes, ValueLimits::default())?)
}

fn fixture(case: Case) -> Result<AtomCatalog, Box<dyn Error>> {
    // Descending source order exercises preparation, not just an already-sorted
    // input. The final Model and index borrow/share this one payload owner.
    let atoms = (0..case.rows)
        .rev()
        .map(|index| {
            let name = format!("p{:03}", index % case.predicates);
            let sign = if index % 7 == 0 {
                Sign::Negative
            } else {
                Sign::Positive
            };
            Ok(Atom::new(
                Predicate::with_sign(name, 1, sign)?,
                vec![value(index, case.depth)?],
            )?)
        })
        .collect::<Result<Vec<_>, Box<dyn Error>>>()?;
    Ok(AtomCatalog::new(atoms))
}

struct Query {
    atom: Atom,
    key: Option<usize>,
    rows: Vec<usize>,
}

fn queries(catalog: &AtomCatalog, positions: &[usize]) -> Result<Vec<Query>, Box<dyn Error>> {
    (0..QUERIES)
        .map(|query| {
            let position = query * catalog.atoms().len() / QUERIES;
            let source = &catalog.atoms()[position];
            // Every fourth key is a genuine missing typed value. Its predicate can
            // still have rows; membership and predicate selection are distinct.
            let atom = if query % 4 == 0 {
                Atom::new(
                    source.predicate().clone(),
                    vec![Value::String("absent".into())],
                )?
            } else {
                source.clone()
            };
            let key = positions
                .iter()
                .copied()
                .find(|&row| catalog.atoms()[row] == atom);
            let rows = positions
                .iter()
                .copied()
                .filter(|&row| catalog.atoms()[row].predicate() == atom.predicate())
                .collect();
            Ok(Query { atom, key, rows })
        })
        .collect()
}

fn checked_queries(
    lookup: AtomLookup<'_, '_>,
    queries: &[Query],
    repeats: usize,
) -> io::Result<(u64, u64)> {
    let mut work = Work::default();
    let mut selected = 0;
    for _ in 0..repeats {
        for query in queries {
            let key = lookup.get_with(&query.atom, || work.tick())?;
            assert_eq!(key.map(zetesis_core::AtomRow::position), query.key);
            let rows = lookup.predicate_with(query.atom.predicate(), || work.tick())?;
            assert_eq!(rows.len(), query.rows.len());
            for (actual, &expected) in rows.zip(&query.rows) {
                work.tick()?; // Consumer row visits are outside binary lookup.
                assert_eq!(actual.position(), expected);
                assert_eq!(actual.atom().predicate(), query.atom.predicate());
                selected += 1;
            }
        }
    }
    Ok((work.0, selected))
}

fn scanned_queries(
    catalog: &AtomCatalog,
    positions: &[usize],
    queries: &[Query],
    repeats: usize,
) -> io::Result<u64> {
    let mut visits = Work::default();
    for _ in 0..repeats {
        for query in queries {
            let mut key = None;
            for &row in positions {
                visits.tick()?;
                if catalog.atoms()[row] == query.atom {
                    key = Some(row);
                    break;
                }
            }
            assert_eq!(key, query.key);
            let mut selected = 0;
            for &row in positions {
                visits.tick()?;
                if catalog.atoms()[row].predicate() == query.atom.predicate() {
                    assert_eq!(row, query.rows[selected]);
                    selected += 1;
                }
            }
            assert_eq!(selected, query.rows.len());
        }
    }
    // These are row visits, not descriptor/byte work: the reference deliberately
    // uses independent derived equality. Do not compare the two numeric units.
    Ok(visits.0)
}

struct Prepared<'a> {
    case: Case,
    catalog: &'a AtomCatalog,
    model: &'a Model,
    index: &'a AtomIndex<'a>,
    index_work: u64,
    index_ns: String,
    model_ns: String,
    timed: bool,
}

impl Prepared<'_> {
    fn report(
        &self,
        out: &mut impl Write,
        route: &str,
        lookup: AtomLookup<'_, '_>,
        positions: &[usize],
    ) -> Result<(), Box<dyn Error>> {
        let queries = queries(self.catalog, positions)?;
        let repeats = if self.timed { REPEATS } else { 1 };
        // Exact full row/order/identity assertions are retained in both paths,
        // outside any timing claim about the library operation in isolation.
        let clock = Clock::new(self.timed);
        let (query_work, selected) = checked_queries(lookup, &queries, repeats)?;
        let query_ns = clock.elapsed();
        let clock = Clock::new(self.timed);
        let scan_visits = scanned_queries(self.catalog, positions, &queries, repeats)?;
        let scan_ns = clock.elapsed();
        writeln!(
            out,
            "{},{},{},{route},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
            self.case.rows,
            self.case.predicates,
            self.case.depth,
            positions.len(),
            repeats,
            self.index_work,
            self.index_ns,
            self.model_ns,
            query_work,
            query_ns,
            selected,
            scan_visits,
            scan_ns,
            self.index.retained_bytes(),
            self.index.preparation_peak_bytes(),
            self.catalog.capacity() as u128 * size_of::<Atom>() as u128,
            self.model.selection_capacity() as u128 * size_of::<usize>() as u128,
            self.model
                .retained_payload_bytes()
                .ok_or_else(|| io::Error::other("payload size overflow"))?,
        )?;
        Ok(())
    }
}

fn run(case: Case, timed: bool, out: &mut impl Write) -> Result<(), Box<dyn Error>> {
    let catalog = fixture(case)?;
    let clock = Clock::new(timed);
    let model = Model::from_positions(&catalog, (0..case.rows).filter(|row| row % 3 != 0))?;
    let model_ns = clock.elapsed();
    let mut work = Work::default();
    let clock = Clock::new(timed);
    let index = AtomIndex::new_with(catalog.atoms(), || work.tick())?;
    let index_ns = clock.elapsed();
    let prepared = Prepared {
        case,
        catalog: &catalog,
        model: &model,
        index: &index,
        index_work: work.0,
        index_ns,
        model_ns,
        timed,
    };
    let original: Vec<_> = (0..case.rows).collect();
    prepared.report(out, "catalog", index.lookup(), &original)?;
    prepared.report(out, "model", model.lookup(), model.positions())?;
    assert!(model.catalog().same_owner(&catalog));
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args().skip(1);
    let timed = match (args.next().as_deref(), args.next()) {
        (None, None) => true,
        (Some("--check"), None) => false,
        _ => return Err(io::Error::other("usage: atom_lookup [--check]").into()),
    };
    let mut out = io::BufWriter::new(io::stdout().lock());
    writeln!(
        out,
        "rows,predicates,depth,route,true_rows,repeats,index_work,index_ns,model_ns,query_work,query_ns,selected_rows,scan_visits,scan_ns,index_retained_bytes,index_peak_bytes,atom_cell_bytes,model_position_bytes,model_conservative_payload_bytes"
    )?;
    for case in CASES {
        run(case, timed, &mut out)?;
    }
    out.flush()?;
    Ok(())
}
