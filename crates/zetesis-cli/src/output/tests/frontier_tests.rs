use super::{Buffer, frontier_statistics, region_statistics};

#[test]
fn unobserved_frontier_is_absent() {
    let mut output = Buffer::new(512);
    frontier_statistics(&mut output, None).unwrap();
    assert_eq!(output.bytes, b"null");
}

#[test]
fn frontier_bytes_preserve_wide_accounting() {
    let bytes = u128::from(u64::MAX) + 1;
    let statistics = zetesis_sat::RegionFrontierStatistics {
        regions: 0,
        capacity: 8,
        retained_bytes: bytes,
        peak_regions: 5,
        peak_capacity: 8,
        peak_retained_bytes: bytes + 32,
    };
    let mut output = Buffer::new(512);
    frontier_statistics(&mut output, Some(statistics)).unwrap();
    let actual = String::from_utf8(output.bytes).unwrap();
    assert_eq!(
        actual,
        format!(
            "{{\"regions\":0,\"capacity\":8,\"retained_bytes\":{bytes},\"peak_regions\":5,\"peak_capacity\":8,\"peak_retained_bytes\":{}}}",
            bytes + 32
        )
    );
}

#[test]
fn frontier_record_respects_the_output_limit() {
    let statistics = zetesis_sat::RegionFrontierStatistics::default();
    let mut output = Buffer::new(4);
    assert!(frontier_statistics(&mut output, Some(statistics)).is_err());
    assert!(output.bytes.len() <= 4);
}

#[test]
fn region_statistics_name_the_producers_frontier_order() {
    for (order, spelling) in [
        (zetesis_sat::FrontierOrder::PerProducer, "per_producer"),
        (zetesis_sat::FrontierOrder::Shared, "shared"),
    ] {
        let mut statistics = zetesis_sat::RegionSearchStatistics::default();
        statistics.frontier_order = Some(order);
        let mut output = Buffer::new(4096);
        region_statistics(&mut output, Some(&statistics)).unwrap();
        let actual = String::from_utf8(output.bytes).unwrap();
        assert!(
            actual.contains(&format!("\"frontier_order\":\"{spelling}\"")),
            "{actual}"
        );
    }
}

#[test]
fn routes_without_producers_report_no_frontier_order() {
    let statistics = zetesis_sat::RegionSearchStatistics::default();
    let mut output = Buffer::new(4096);
    region_statistics(&mut output, Some(&statistics)).unwrap();
    let actual = String::from_utf8(output.bytes).unwrap();
    assert!(actual.contains("\"frontier_order\":null"), "{actual}");
}
