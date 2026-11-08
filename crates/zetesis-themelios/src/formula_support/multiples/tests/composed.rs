#[test]
fn equal_populations_use_the_intersection_in_the_numeric_pipeline() {
    for body in ["candidate(N),candidate(D)", "candidate(D),candidate(N)"] {
        let source = format!("candidate(2..30). h(N,D) :- {body},D<N,N\\D=0.");
        let (ordinary, selected) = super::ranges::numeric(&source);
        assert!(selected.owned);
        assert!(ordinary.work.range_probes.unwrap() > 0);
        assert_eq!(selected.work.range_probes, Some(0));
        assert!(selected.work.join_rows.unwrap() < ordinary.work.join_rows.unwrap());
        let expected = (2..=30)
            .flat_map(|n| (2..n).filter(move |d| n % d == 0))
            .count();
        assert_eq!(selected.values.len(), expected);
    }
}
