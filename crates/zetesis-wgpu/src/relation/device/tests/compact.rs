use super::*;

mod mixed;

fn packed<T: Copy + Into<u32>>(values: &[T], bits: u32) -> Vec<u32> {
    let mut bytes = vec![0xff; values.len().div_ceil((32 / bits) as usize) * 4];
    pack_cells(
        values,
        bits,
        wgpu::WriteOnly::from_mut(bytes.as_mut_slice()),
        &mut || Ok(()),
    )
    .unwrap();
    bytes
        .chunks_exact(4)
        .map(|word| u32::from_le_bytes(word.try_into().unwrap()))
        .collect()
}

fn roundtrip<T: Copy + Into<u32>>(values: &[T], bits: u32) {
    let words = packed(values, bits);
    let lanes = (32 / bits) as usize;
    let decoded: Vec<_> = (0..values.len())
        .map(|row| {
            (words[row / lanes] >> ((row % lanes) * bits as usize)) & (u32::MAX >> (32 - bits))
        })
        .collect();
    assert_eq!(
        decoded,
        values.iter().copied().map(Into::into).collect::<Vec<u32>>()
    );
}

#[test]
fn packed_cell_roundtrip_preserves_identifiers() {
    roundtrip(&[0_u8, 1, 254, 255, 13], 8);
    roundtrip(&[0_u16, 65_535, 256], 16);
    roundtrip(&[0_u32, 65_536, u32::MAX], 32);
}

#[test]
fn unused_payload_lanes_are_zero() {
    assert_eq!(packed(&[0_u8, 1, 254, 255, 13], 8)[1] >> 8, 0);
    assert_eq!(packed(&[0_u16, 65_535, 256], 16)[1] >> 16, 0);
    assert!(packed::<u8>(&[], 8).is_empty());
}

#[test]
fn uploaded_headers_address_the_original_columns() {
    let predicate = Predicate::new("row", 2).unwrap();
    let atoms = [[0, 10], [1, 11], [0, 12]]
        .map(|row| Atom::new(predicate.clone(), row.map(Value::Number).to_vec()).unwrap());
    let relation = Relation::from_atoms(&predicate, &atoms, Limits::default()).unwrap();
    let bytes = packing::column_bytes(&relation, &wgpu::Limits::default()).unwrap();
    let mut target = vec![0xff; usize::try_from(bytes).unwrap()];
    pack_columns(
        &relation,
        wgpu::WriteOnly::from_mut(target.as_mut_slice()),
        &mut || Ok(()),
    )
    .unwrap();
    let words: Vec<_> = target
        .chunks_exact(4)
        .map(|word| u32::from_le_bytes(word.try_into().unwrap()))
        .collect();
    assert_eq!(&words[..4], [4, 8, 5, 8]);
    for (column, values) in relation.columns().enumerate() {
        let first = words[column * 2] as usize;
        let bits = words[column * 2 + 1];
        let lanes = (32 / bits) as usize;
        for (row, id) in values.iter().enumerate() {
            let decoded = (words[first + row / lanes] >> ((row % lanes) * bits as usize))
                & (u32::MAX >> (32 - bits));
            assert_eq!(decoded, id);
        }
        assert_eq!(words[first] >> (relation.row_count() * bits as usize), 0);
    }
}

#[test]
fn nullary_upload_retains_one_padding_word() {
    let predicate = Predicate::new("row", 0).unwrap();
    let atoms = [Atom::new(predicate.clone(), vec![]).unwrap()];
    let relation = Relation::from_atoms(&predicate, &atoms, Limits::default()).unwrap();
    assert_eq!(
        packing::column_bytes(&relation, &wgpu::Limits::default()).unwrap(),
        4
    );
    let mut target = [0xff; 4];
    pack_columns(
        &relation,
        wgpu::WriteOnly::from_mut(target.as_mut_slice()),
        &mut || Ok(()),
    )
    .unwrap();
    assert_eq!(target, [0; 4]);
}

#[test]
fn packed_payload_copy_observes_mid_column_interruption() {
    let values = vec![255_u8; PACK_CONTROL_BYTES + 1];
    let mut target = vec![0; values.len().div_ceil(4) * 4];
    let mut polls = 0;
    let result = pack_cells(
        &values,
        8,
        wgpu::WriteOnly::from_mut(target.as_mut_slice()),
        &mut || {
            polls += 1;
            if polls == 2 {
                Err(GpuError::interrupted(Stop::Cancelled))
            } else {
                Ok(())
            }
        },
    );
    assert!(result.is_err());
    assert_eq!(polls, 2);
    assert_eq!(&target[..PACK_CONTROL_BYTES], vec![255; PACK_CONTROL_BYTES]);
    assert_eq!(&target[PACK_CONTROL_BYTES..], [0; 4]);
}

fn physical_widths(backend: GpuApi) {
    let mut executor =
        GpuRelationExecutor::new_selected(GpuOptions::default(), GpuSelection { api: backend })
            .unwrap();
    assert!(executor.info().is_hardware_gpu());
    assert_eq!(executor.info().backend(), backend.name());
    for (rows, bits) in [(3, 8), (129, 16), (32_769, 32)] {
        let predicate = Predicate::new("row", 2).unwrap();
        let atoms: Vec<_> = (0..rows)
            .map(|row| {
                Atom::new(
                    predicate.clone(),
                    vec![Value::Number(row * 2), Value::Number(row * 2 + 1)],
                )
                .unwrap()
            })
            .collect();
        let relation = Relation::from_atoms(&predicate, &atoms, Limits::default()).unwrap();
        assert!(relation.columns().all(|column| column.bits() == bits));
        let value = Value::Number((rows - 1) * 2);
        let queries = [relation
            .query(&[(0, (&value).into())], Limits::default())
            .unwrap()];
        let all = relation.all(Limits::default()).unwrap();
        let expected = relation
            .select_mask(&queries[0], &all, Limits::default())
            .unwrap();
        let mut prepared = executor
            .prepare(
                &relation,
                RelationGpuLimits::default(),
                &Cancellation::default(),
            )
            .unwrap();
        let actual = prepared
            .filter(
                &queries,
                RelationGpuLimits::default(),
                &Cancellation::default(),
            )
            .unwrap();
        assert_eq!(
            actual.selection(0, Limits::default()).unwrap().positions(),
            [usize::try_from(rows - 1).unwrap()]
        );
        assert_eq!(
            actual.selection(0, Limits::default()).unwrap().positions(),
            relation
                .selection_from_mask(expected.words(), Limits::default())
                .unwrap()
                .positions()
        );
    }
    mixed::physical(&mut executor);
}

#[test]
#[ignore = "requires Metal: compact relation widths match CPU"]
fn metal_compact_widths_match_cpu() {
    physical_widths(GpuApi::Metal);
}

#[test]
#[ignore = "requires Vulkan: compact relation widths match CPU"]
fn vulkan_compact_widths_match_cpu() {
    physical_widths(GpuApi::Vulkan);
}
