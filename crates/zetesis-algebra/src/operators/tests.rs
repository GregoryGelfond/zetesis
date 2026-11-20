use std::collections::BTreeSet;

use super::{
    Atoms, Cpu, EnumerationStats, RelationalOps, Schedule, Template, least, project, rows, union,
};

fn support() -> Atoms {
    Atoms::empty(1)
}

fn sieve_template(n: i32, schedule: Schedule) -> Template {
    Template::new(2)
        .with_schedule(schedule)
        .with_range(0, 1, n)
        .unwrap()
        .with_dependent_range(1, 2, 0, -1)
        .unwrap()
        .with_remainder(0, 1)
        .unwrap()
        .with_head_slot(0)
        .unwrap()
}

fn run_rows(template: &Template) -> (Vec<Vec<i32>>, EnumerationStats) {
    let mut stats = EnumerationStats::default();
    let bindings = rows(template, &support(), &mut |_| Ok::<(), ()>(()), &mut stats).unwrap();
    (bindings.to_rows(), stats)
}

fn composites(n: i32) -> BTreeSet<i32> {
    let mut set = BTreeSet::new();
    for x in 1..=n {
        for y in 2..x {
            if y != 0 && x % y == 0 {
                set.insert(x);
                break;
            }
        }
    }
    set
}

fn project_heads(template: &Template, rows: &[Vec<i32>]) -> BTreeSet<i32> {
    let bindings = super::Bindings::from_columns(if rows.is_empty() {
        vec![Vec::new(); template.width()]
    } else {
        let width = rows[0].len();
        let mut columns = vec![Vec::with_capacity(rows.len()); width];
        for row in rows {
            for (column, value) in columns.iter_mut().zip(row) {
                column.push(*value);
            }
        }
        columns
    })
    .unwrap();
    project(template, &bindings, &mut |_| Ok::<(), ()>(()))
        .unwrap()
        .unwrap()
        .column(0)
        .iter()
        .copied()
        .collect()
}

#[test]
fn union_is_commutative_and_idempotent() {
    let left = Atoms::from_sorted_unique(vec![vec![1, 3, 5]]).unwrap();
    let right = Atoms::from_sorted_unique(vec![vec![3, 4, 5, 8]]).unwrap();
    let ab = union(&left, &right, &mut |_| Ok::<(), ()>(())).unwrap();
    let ba = union(&right, &left, &mut |_| Ok::<(), ()>(())).unwrap();
    assert_eq!(ab, ba);
    assert_eq!(ab.column(0), [1, 3, 4, 5, 8]);
    assert_eq!(union(&ab, &ab, &mut |_| Ok::<(), ()>(())).unwrap(), ab);
}

#[test]
fn project_coalesces_duplicate_heads() {
    let template = Template::new(2).with_head_slot(0).unwrap();
    let bindings = super::Bindings::from_columns(vec![vec![2, 2, 4], vec![1, 2, 1]]).unwrap();
    let atoms = project(&template, &bindings, &mut |_| Ok::<(), ()>(()))
        .unwrap()
        .unwrap();
    assert_eq!(atoms.column(0), [2, 4]);
}

#[test]
fn bind_is_monotone_in_a_fact_domain() {
    let small = Template::new(1)
        .with_facts(0, vec![2, 4])
        .unwrap()
        .with_head_slot(0)
        .unwrap();
    let large = Template::new(1)
        .with_facts(0, vec![2, 3, 4])
        .unwrap()
        .with_head_slot(0)
        .unwrap();
    let (small_rows, _) = run_rows(&small);
    let (large_rows, _) = run_rows(&large);
    let small_set: BTreeSet<_> = small_rows.into_iter().collect();
    let large_set: BTreeSet<_> = large_rows.into_iter().collect();
    assert!(small_set.is_subset(&large_set));
    assert_eq!(large_set.len(), 3);
}

#[test]
fn least_enumerates_a_simple_chain() {
    let atoms = least(
        1,
        |current, account| {
            let mut values = current.column(0).to_vec();
            if current.is_empty() {
                account(1)?;
                values.push(0);
            } else {
                let last = *current.column(0).last().unwrap();
                account(1)?;
                if let Some(next) = last.checked_add(2)
                    && next <= 10
                {
                    values.push(next);
                }
            }
            Ok(Atoms::from_unsorted(&[values], account).unwrap().unwrap())
        },
        &mut |_| Ok::<(), ()>(()),
    )
    .unwrap();
    assert_eq!(atoms.column(0), [0, 2, 4, 6, 8, 10]);
}

#[test]
fn empty_body_bind_is_a_single_row() {
    let template = Template::new(0);
    let (rows, stats) = run_rows(&template);
    assert_eq!(rows, vec![Vec::<i32>::new()]);
    assert_eq!(stats.retained, 1);
    assert_eq!(stats.candidates, 1);
}

fn same_bindings(left: &[Vec<i32>], right: &[Vec<i32>]) {
    let left_set: BTreeSet<_> = left.iter().cloned().collect();
    let right_set: BTreeSet<_> = right.iter().cloned().collect();
    assert_eq!(left_set, right_set);
}

#[test]
fn remainder_filter_only_and_generating_agree_on_the_sieve() {
    for n in [20, 100, 1_000] {
        let filter_only = sieve_template(n, Schedule::FilterOnly);
        let generating = sieve_template(n, Schedule::Generating);
        let (filter_rows, filter_stats) = run_rows(&filter_only);
        let (gen_rows, gen_stats) = run_rows(&generating);
        same_bindings(&filter_rows, &gen_rows);
        let heads = project_heads(&filter_only, &filter_rows);
        assert_eq!(heads, composites(n), "n = {n}");
        assert_eq!(
            project_heads(&generating, &gen_rows),
            heads,
            "projected n = {n}"
        );
        assert!(
            gen_stats.candidates < filter_stats.candidates,
            "n = {n}: generating {} vs filter-only {}",
            gen_stats.candidates,
            filter_stats.candidates
        );
        assert_eq!(filter_stats.retained, gen_stats.retained);
        // Filter-only walks Y = 2..X-1 for every X, which is n(n-1)/2 - (n-1)
        // after the empty inner ranges at X < 3. The generating path should
        // stay near the number of actual multiples, not the product.
        // The exact walk count for X in 1..=n, Y in 2..=X-1 is sum_{x=3..n}(x-2)
        // = (n-2)(n-1)/2.
        let walked = u64::try_from((n - 2) * (n - 1) / 2).unwrap();
        assert_eq!(filter_stats.candidates, walked, "n = {n}");
        println!(
            "sieve n={n}: filter-only candidates={} retained={} evals={}; generating candidates={} retained={} evals={}",
            filter_stats.candidates,
            filter_stats.retained,
            filter_stats.expression_evaluations,
            gen_stats.candidates,
            gen_stats.retained,
            gen_stats.expression_evaluations
        );
    }
}

#[test]
fn remainder_work_at_five_thousand_drops_with_generating_bind() {
    let n = 5_000i32;
    let (filter_rows, filter_stats) = run_rows(&sieve_template(n, Schedule::FilterOnly));
    let (gen_rows, gen_stats) = run_rows(&sieve_template(n, Schedule::Generating));
    same_bindings(&filter_rows, &gen_rows);
    assert_eq!(
        project_heads(&sieve_template(n, Schedule::Generating), &gen_rows),
        composites(n)
    );
    assert_eq!(
        filter_stats.candidates,
        u64::try_from((n - 2) * (n - 1) / 2).unwrap(),
    );
    assert!(gen_stats.candidates * 20 < filter_stats.candidates);
    println!(
        "sieve n=5000: filter-only candidates={} retained={}; generating candidates={} retained={}",
        filter_stats.candidates, filter_stats.retained, gen_stats.candidates, gen_stats.retained
    );
}

#[test]
fn failed_pairs_are_not_retained() {
    let template = sieve_template(20, Schedule::FilterOnly);
    let (rows, stats) = run_rows(&template);
    assert_eq!(u64::try_from(rows.len()).unwrap(), stats.retained);
    assert!(stats.retained < stats.candidates);
    for row in &rows {
        assert_eq!(row[0] % row[1], 0);
    }
}

#[test]
fn plus_filter_only_and_generating_agree() {
    // X, Y in 0..=10, Z = X+Y, Z in 0..=20.
    let filter_only = Template::new(3)
        .with_schedule(Schedule::FilterOnly)
        .with_range(0, 0, 10)
        .unwrap()
        .with_range(1, 0, 10)
        .unwrap()
        .with_range(2, 0, 20)
        .unwrap()
        .with_plus(0, 1, 2)
        .unwrap();
    let generating = filter_only.clone().with_schedule(Schedule::Generating);
    let (filter_rows, filter_stats) = run_rows(&filter_only);
    let (gen_rows, gen_stats) = run_rows(&generating);
    same_bindings(&filter_rows, &gen_rows);
    assert_eq!(filter_rows.len(), 11 * 11);
    assert!(gen_stats.candidates < filter_stats.candidates);
    println!(
        "plus: filter-only candidates={} retained={}; generating candidates={} retained={}",
        filter_stats.candidates, filter_stats.retained, gen_stats.candidates, gen_stats.retained
    );
}

#[test]
fn times_filter_only_and_generating_agree() {
    let filter_only = Template::new(3)
        .with_schedule(Schedule::FilterOnly)
        .with_range(0, 1, 8)
        .unwrap()
        .with_range(1, 1, 8)
        .unwrap()
        .with_range(2, 1, 64)
        .unwrap()
        .with_times(0, 1, 2)
        .unwrap();
    let generating = filter_only.clone().with_schedule(Schedule::Generating);
    let (filter_rows, filter_stats) = run_rows(&filter_only);
    let (gen_rows, gen_stats) = run_rows(&generating);
    same_bindings(&filter_rows, &gen_rows);
    assert_eq!(filter_rows.len(), 8 * 8);
    assert!(gen_stats.candidates < filter_stats.candidates);
    println!(
        "times: filter-only candidates={} retained={}; generating candidates={} retained={}",
        filter_stats.candidates, filter_stats.retained, gen_stats.candidates, gen_stats.retained
    );
}

#[test]
fn abs_filter_only_and_generating_agree() {
    let filter_only = Template::new(2)
        .with_schedule(Schedule::FilterOnly)
        .with_range(0, -5, 5)
        .unwrap()
        .with_range(1, 0, 5)
        .unwrap()
        .with_abs(0, 1)
        .unwrap();
    let generating = filter_only.clone().with_schedule(Schedule::Generating);
    let (filter_rows, _) = run_rows(&filter_only);
    let (gen_rows, _) = run_rows(&generating);
    same_bindings(&filter_rows, &gen_rows);
    assert_eq!(filter_rows.len(), 11); // every X in -5..=5 has a unique |X|
}

#[test]
fn quotient_filter_only_and_generating_agree() {
    let filter_only = Template::new(3)
        .with_schedule(Schedule::FilterOnly)
        .with_range(0, 1, 20)
        .unwrap()
        .with_range(1, 1, 6)
        .unwrap()
        .with_range(2, 0, 20)
        .unwrap()
        .with_quotient(0, 1, 2)
        .unwrap();
    let generating = filter_only.clone().with_schedule(Schedule::Generating);
    let (filter_rows, filter_stats) = run_rows(&filter_only);
    let (gen_rows, gen_stats) = run_rows(&generating);
    same_bindings(&filter_rows, &gen_rows);
    for row in &filter_rows {
        assert_eq!(row[0] / row[1], row[2]);
    }
    assert!(gen_stats.candidates <= filter_stats.candidates);
    println!(
        "quotient: filter-only candidates={} retained={}; generating candidates={} retained={}",
        filter_stats.candidates, filter_stats.retained, gen_stats.candidates, gen_stats.retained
    );
}

#[test]
fn guerins_num_domains_agree_under_both_schedules() {
    let n = 200i32;
    let nums: Vec<i32> = (2..=n).collect();
    let filter_only = Template::new(2)
        .with_schedule(Schedule::FilterOnly)
        .with_facts(0, nums.clone())
        .unwrap()
        .with_facts(1, nums)
        .unwrap()
        .with_remainder(0, 1)
        .unwrap()
        .with_ne(0, 1)
        .unwrap()
        .with_head_slot(0)
        .unwrap();
    let generating = filter_only.clone().with_schedule(Schedule::Generating);
    let (filter_rows, filter_stats) = run_rows(&filter_only);
    let (gen_rows, gen_stats) = run_rows(&generating);
    same_bindings(&filter_rows, &gen_rows);
    assert_eq!(project_heads(&generating, &gen_rows), composites(n));
    assert!(gen_stats.candidates < filter_stats.candidates);
    println!(
        "guerin n=200: filter-only candidates={} retained={}; generating candidates={} retained={}",
        filter_stats.candidates, filter_stats.retained, gen_stats.candidates, gen_stats.retained
    );
}

#[test]
fn cpu_trait_matches_free_functions() {
    let template = sieve_template(30, Schedule::Generating);
    let cpu = Cpu;
    let mut stats = EnumerationStats::default();
    let via_fn = rows(&template, &support(), &mut |_| Ok::<(), ()>(()), &mut stats).unwrap();
    let via_trait =
        RelationalOps::bind(&cpu, &template, &support(), &mut |_| Ok::<(), ()>(())).unwrap();
    assert_eq!(via_fn, via_trait);
}

#[test]
fn column_layout_is_packed_i32() {
    let layout = super::ColumnLayout::packed(2, 100).unwrap();
    assert_eq!(layout.width(), 2);
    assert_eq!(layout.rows(), 100);
    assert_eq!(layout.column_stride(), 400);
}

#[test]
fn refusal_returns_no_partial_union() {
    let left = Atoms::from_sorted_unique(vec![vec![1, 2]]).unwrap();
    let right = Atoms::from_sorted_unique(vec![vec![2, 3]]).unwrap();
    let mut remaining = 1u64;
    let result = union(&left, &right, &mut |amount| {
        if remaining < amount {
            return Err("stopped");
        }
        remaining -= amount;
        Ok(())
    });
    assert_eq!(result, Err("stopped"));
}

#[test]
fn overflow_and_undefined_arithmetic_drop_the_row() {
    // i32::MIN / -1 overflows; 1 / 0 is undefined. Neither row is retained.
    let template = Template::new(3)
        .with_facts(0, vec![i32::MIN, 1])
        .unwrap()
        .with_facts(1, vec![-1, 0])
        .unwrap()
        .with_facts(2, vec![0])
        .unwrap()
        .with_quotient(0, 1, 2)
        .unwrap();
    let (rows, stats) = run_rows(&template);
    assert!(rows.is_empty());
    assert_eq!(stats.retained, 0);
}

#[test]
fn sparse_facts_are_not_a_dense_generating_range() {
    // The hull of {1, i32::MAX} is not the interval 1..=MAX. Generating
    // multiples of 2 through that hull would visit more than a billion
    // integers the relation does not contain.
    let filter_only = Template::new(2)
        .with_schedule(Schedule::FilterOnly)
        .with_facts(0, vec![1, i32::MAX])
        .unwrap()
        .with_facts(1, vec![2])
        .unwrap()
        .with_remainder(0, 1)
        .unwrap()
        .with_head_slot(0)
        .unwrap();
    let generating = filter_only.clone().with_schedule(Schedule::Generating);
    let (filter_rows, filter_stats) = run_rows(&filter_only);
    let (gen_rows, gen_stats) = run_rows(&generating);
    same_bindings(&filter_rows, &gen_rows);
    assert!(filter_rows.is_empty());
    assert!(
        gen_stats.candidates <= 4,
        "generating candidates={}",
        gen_stats.candidates
    );
    assert!(
        filter_stats.candidates <= 4,
        "filter-only candidates={}",
        filter_stats.candidates
    );
    let _ = (filter_stats, gen_stats);
}
