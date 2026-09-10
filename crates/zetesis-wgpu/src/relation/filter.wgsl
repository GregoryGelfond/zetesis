// Equality IDs belong to the prepared relation's dictionary. They carry no
// arithmetic, term-order or candidate-membership interpretation.
struct Dimensions {
    rows: u32, columns: u32, queries: u32, words: u32,
    epoch: u32, reserved0: u32, reserved1: u32, reserved2: u32,
}
struct Query { first: u32, count: u32, possible: u32, work: u32 }
struct Equality { column: u32, value: u32 }
@group(0) @binding(0) var<uniform> shape: Dimensions;
@group(0) @binding(1) var<storage, read> columns: array<u32>;
@group(0) @binding(2) var<storage, read> queries: array<Query>;
@group(0) @binding(3) var<storage, read> equalities: array<Equality>;
@group(0) @binding(4) var<storage, read_write> output: array<u32>;

const ROWS_PER_GROUP: u32 = 64u;
const BITS_PER_WORD: u32 = 32u;
const RECEIPT_WORDS: u32 = 4u;
const RECEIPT_MARKER: u32 = 0x434f4c31u;
var<workgroup> matches: array<u32, ROWS_PER_GROUP>;

@compute @workgroup_size(ROWS_PER_GROUP)
fn select_rows(@builtin(workgroup_id) group: vec3<u32>,
          @builtin(local_invocation_id) local: vec3<u32>) {
    let query = queries[group.y];
    let row = group.x * ROWS_PER_GROUP + local.x;
    var accepted = row < shape.rows && query.possible != 0u;
    if row < shape.rows {
        // Materialize each comparison even after an earlier mismatch. This
        // retains the declared full-scan equality-work accounting.
        for (var offset = 0u; offset < query.count; offset++) {
            let equality = equalities[query.first + offset];
            let equal = columns[equality.column * shape.rows + row] == equality.value;
            accepted = accepted && equal;
        }
    }
    matches[local.x] = select(0u, 1u, accepted);
    // Tail lanes and failed rows reach the same barrier as all other lanes.
    workgroupBarrier();
    let first = group.y * (RECEIPT_WORDS + shape.words);
    if local.x == 0u && group.x == 0u {
        output[first] = RECEIPT_MARKER;
        output[first + 1u] = shape.epoch;
        output[first + 2u] = group.y;
        output[first + 3u] = query.work;
    }
    if local.x % BITS_PER_WORD == 0u {
        let word = group.x * 2u + local.x / BITS_PER_WORD;
        if word < shape.words {
            var bits = 0u;
            for (var bit = 0u; bit < BITS_PER_WORD; bit++) {
                bits |= matches[local.x + bit] << bit;
            }
            output[first + RECEIPT_WORDS + word] = bits;
        }
    }
}
