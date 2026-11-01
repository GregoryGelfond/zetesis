//! Outputs are signed: a required presence holds its open atom, refutes a
//! region that cut it, and forces a failing condition, as a required absence
//! does with the signs exchanged.

use super::*;

/// a and y give x: a needs x or not y. Only possible foundedness derives
/// this, so a carrier without producers relies on the row.
fn presence_row(program: &Program) -> NarrowingScratch {
    let mut scratch = NarrowingScratch::default();
    scratch.learning.explanations.prepare(&program.owner);
    assert!(add(
        &mut scratch.learning.explanations,
        &[(0, true), (3, true)],
        &[(2, true)]
    ));
    scratch
}

fn carrier(
    program: &Program,
    region: &mut Region,
    scratch: &mut NarrowingScratch,
) -> (Narrowing, NarrowingStatistics) {
    let attempt = program.index.narrow_known(
        OriginalSubject::carrier(&program.owner, None),
        region,
        &mut program.index.knowledge(),
        scratch,
        RegionLimits::default(),
        &Cancellation::default(),
    );
    (attempt.result.unwrap(), attempt.statistics)
}

#[test]
fn a_presence_output_holds_its_open_atom() {
    let program = Program::new(false);
    let initial = region(&[(0, true), (3, true)]);
    let mut without_row = initial.clone();
    carrier(&program, &mut without_row, &mut NarrowingScratch::default());
    assert_eq!(without_row.decision(2), None);
    let mut with_row = initial;
    let (result, counts) = carrier(&program, &mut with_row, &mut presence_row(&program));
    assert!(matches!(result, Narrowing::Fixed { .. }));
    assert_eq!(with_row.decision(2), Some(true));
    assert_eq!(counts.support_explanations.matched, 1);
    assert_eq!(counts.support_explanations.cut, 0);
}

#[test]
fn a_cut_presence_output_refutes_a_complete_condition() {
    let program = Program::new(false);
    let initial = region(&[(0, true), (2, false), (3, true)]);
    let (without_row, _) = carrier(
        &program,
        &mut initial.clone(),
        &mut NarrowingScratch::default(),
    );
    assert!(matches!(without_row, Narrowing::Fixed { .. }));
    let (with_row, counts) = carrier(&program, &mut initial.clone(), &mut presence_row(&program));
    assert_eq!(with_row, Narrowing::Refuted);
    assert_eq!(counts.support_explanations.refuted, 1);
}

#[test]
fn a_cut_presence_output_forces_the_open_condition_to_fail() {
    let program = Program::new(false);
    let initial = region(&[(2, false), (3, true)]);
    let mut without_row = initial.clone();
    carrier(&program, &mut without_row, &mut NarrowingScratch::default());
    assert_eq!(without_row.decision(0), None);
    let mut with_row = initial;
    let (result, counts) = carrier(&program, &mut with_row, &mut presence_row(&program));
    assert!(matches!(result, Narrowing::Fixed { .. }));
    assert_eq!(with_row.decision(0), Some(false));
    assert_eq!(counts.support_explanations.forced, 1);
}

#[test]
fn presence_rows_preserve_every_answer_in_all_partial_regions() {
    let program = Program::new(false);
    let answers: Vec<_> = (0..32)
        .filter(|&mask| {
            crate::check(
                &program.theory,
                &world(&program.theory, mask),
                crate::Limits::default(),
                &Cancellation::default(),
            )
            .unwrap()
            .accepted()
        })
        .collect();
    let mut scratch = presence_row(&program);
    let (mut matched, mut forced) = (0, 0);
    for mut code in 0..243 {
        let mut initial = Region::all_open(5);
        for atom in 0..5 {
            match code % 3 {
                1 => assert!(initial.cut(atom)),
                2 => assert!(initial.hold(atom)),
                _ => {}
            }
            code /= 3;
        }
        let mut narrowed = initial.clone();
        let (result, counts) = carrier(&program, &mut narrowed, &mut scratch);
        matched += counts.support_explanations.matched;
        forced += counts.support_explanations.forced;
        for &answer in &answers {
            if compatible(&initial, answer) {
                assert_ne!(result, Narrowing::Refuted);
                assert!(compatible(&narrowed, answer));
            }
        }
    }
    assert!(matched > 0, "complete matches must hold the output");
    assert!(forced > 0, "partial matches must force the open condition");
}
