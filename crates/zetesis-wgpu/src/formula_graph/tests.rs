use super::*;
use zetesis_ferraris::{AdmissionLimits, FormulaParts, Node, OperandSpan};

fn theory() -> Theory {
    Theory::new(
        2,
        FormulaParts::new(
            vec![
                Node::atom(0),
                Node::atom(1),
                Node::and_span(OperandSpan {
                    start: 1,
                    length: 3,
                }),
                Node::or_span(OperandSpan {
                    start: 2,
                    length: 3,
                }),
            ],
            vec![usize::MAX, 0, 1, 0, 2, usize::MAX],
        )
        .unwrap(),
        vec![3],
        AdmissionLimits::default(),
    )
    .unwrap()
}

#[test]
fn combined_word_addresses_are_checked_before_device_bytes() {
    assert_eq!(bytes(0, 0).unwrap(), 16);
    assert_eq!(bytes(u32::MAX / 4, 3).unwrap(), u64::from(u32::MAX) * 4);
    assert_eq!(
        bytes(u32::MAX / 4, 4).unwrap_err().kind(),
        GpuErrorKind::Capacity
    );
    assert_eq!(
        bytes(u32::MAX / 4 + 1, 0).unwrap_err().kind(),
        GpuErrorKind::Capacity
    );
}

#[test]
fn logical_rows_omit_unused_cells_and_preserve_overlap_occurrences() {
    let theory = theory();
    let shape = Shape::new(&theory, &wgpu::Limits::default()).unwrap();
    assert_eq!((shape.edges, shape.wide_words, shape.bytes), (6, 6, 88));
    let mut packed = Vec::new();
    let mut tail = 0;
    for index in 0..theory.view().len() {
        packed.extend(header(theory.view().node(index).unwrap(), &mut tail).unwrap());
    }
    append_operands(&theory, &mut packed, &Cancellation::default()).unwrap();
    assert_eq!(&packed[8..16], &[5, 0, 3, 0, 6, 3, 3, 0]);
    assert_eq!(&packed[16..], &[0, 1, 0, 1, 0, 2]);
    assert_eq!(packed.len() as u64 * 4, shape.bytes);
}

#[test]
fn operand_tail_is_included_in_both_device_byte_limits() {
    for buffer in [true, false] {
        for (limit, admitted) in [(87, false), (88, true)] {
            let mut device = wgpu::Limits::default();
            if buffer {
                device.max_buffer_size = limit;
            } else {
                device.max_storage_buffer_binding_size = limit;
            }
            assert_eq!(Shape::new(&theory(), &device).is_ok(), admitted);
        }
    }
}

#[test]
fn cancelled_operand_packing_returns_a_typed_interruption() {
    let cancellation = Cancellation::default();
    cancellation.cancel();
    assert_eq!(
        append_operands(&theory(), &mut Vec::new(), &cancellation)
            .unwrap_err()
            .interruption(),
        Some(zetesis_cpu::Stop::Cancelled)
    );
}
