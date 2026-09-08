// One workgroup owns an occurrence. No floating point or optional subgroup ops.
struct Dimensions { tuples: u32, guards: u32, worlds: u32, function: u32, stride: u32, epoch: u32, work: u32, reserved: u32 }
@group(0) @binding(0) var<uniform> dimensions: Dimensions;
@group(0) @binding(1) var<storage, read> contributions: array<u32>;
@group(0) @binding(2) var<storage, read> guards: array<u32>;
@group(0) @binding(3) var<storage, read> masks: array<u32>;
@group(0) @binding(4) var<storage, read_write> output: array<u32>;

const MINIMUM: u32 = 3u;
const MAXIMUM: u32 = 4u;
const EQ: u32 = 0u;
const NE: u32 = 1u;
const LT: u32 = 2u;
const LE: u32 = 3u;
const GT: u32 = 4u;
const GE: u32 = 5u;
var<workgroup> original_values: array<i32, 64>;
var<workgroup> frozen_values: array<i32, 64>;
var<workgroup> original_present: array<u32, 64>;
var<workgroup> frozen_present: array<u32, 64>;

fn combine(left: i32, right: i32, lp: u32, rp: u32) -> i32 {
    if dimensions.function < MINIMUM { return left + right; }
    if rp == 0u { return left; }
    if lp == 0u { return right; }
    if dimensions.function == MINIMUM { return min(left, right); }
    return max(left, right);
}

fn guards_hold(value: i32, present: u32) -> u32 {
    var holds = true;
    for (var guard = 0u; guard < dimensions.guards; guard += 1u) {
        let operation = guards[guard * 2u];
        let bound = bitcast<i32>(guards[guard * 2u + 1u]);
        var less = value < bound;
        var equal = value == bound;
        if present == 0u {
            // Empty extrema are genuine endpoints, not sentinel integers.
            less = dimensions.function == MAXIMUM;
            equal = false;
        }
        var truth = false;
        switch operation {
            case EQ: { truth = equal; }
            case NE: { truth = !equal; }
            case LT: { truth = less; }
            case LE: { truth = less || equal; }
            case GT: { truth = !less && !equal; }
            case GE: { truth = !less; }
            default: {}
        }
        holds = holds && truth;
    }
    return select(0u, 1u, holds);
}

@compute @workgroup_size(64)
fn reduce(@builtin(workgroup_id) group: vec3<u32>, @builtin(local_invocation_index) lane: u32) {
    let row = group.x * dimensions.stride;
    let mask_words = (dimensions.stride - 1u) / 2u;
    var ov = 0;
    var fv = 0;
    // Numerical sums/counts have zero even for an empty carrier.
    var op = select(0u, 1u, dimensions.function < MINIMUM);
    var fp = op;
    for (var tuple = lane; tuple < dimensions.tuples; tuple += 64u) {
        let value = bitcast<i32>(contributions[tuple * 2u]);
        let present = contributions[tuple * 2u + 1u];
        let bit = 1u << (tuple % 32u);
        let original = (masks[row + 1u + tuple / 32u] & bit) != 0u;
        let frozen = (masks[row + 1u + mask_words + tuple / 32u] & bit) != 0u;
        if original && present != 0u { ov = combine(ov, value, op, present); op = 1u; }
        if frozen && present != 0u { fv = combine(fv, value, fp, present); fp = 1u; }
    }
    original_values[lane] = ov;
    frozen_values[lane] = fv;
    original_present[lane] = op;
    frozen_present[lane] = fp;
    workgroupBarrier();
    for (var distance = 32u; distance > 0u; distance /= 2u) {
        if lane < distance {
            original_values[lane] = combine(original_values[lane], original_values[lane + distance], original_present[lane], original_present[lane + distance]);
            frozen_values[lane] = combine(frozen_values[lane], frozen_values[lane + distance], frozen_present[lane], frozen_present[lane + distance]);
            original_present[lane] |= original_present[lane + distance];
            frozen_present[lane] |= frozen_present[lane + distance];
        }
        workgroupBarrier();
    }
    if lane == 0u {
        let result = group.x * 10u;
        output[result] = dimensions.epoch;
        output[result + 1u] = group.x;
        output[result + 2u] = masks[row];
        output[result + 3u] = bitcast<u32>(original_values[0]);
        output[result + 4u] = original_present[0];
        output[result + 5u] = guards_hold(original_values[0], original_present[0]);
        output[result + 6u] = bitcast<u32>(frozen_values[0]);
        output[result + 7u] = frozen_present[0];
        output[result + 8u] = guards_hold(frozen_values[0], frozen_present[0]);
        output[result + 9u] = dimensions.work;
    }
}
