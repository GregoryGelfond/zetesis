// Optional integer instructions, preserving the same four-word wire values.
// Two u64 limbs implement the complete admitted nonnegative i128 range; this
// does not narrow a value or assume that a sum fits in one machine integer.
fn half64(value: vec2<u32>) -> u64 {
    return u64(value.x) | (u64(value.y) << 32u);
}
fn words64(low: u64, high: u64) -> vec4<u32> {
    return vec4<u32>(u32(low), u32(low >> 32u), u32(high), u32(high >> 32u));
}
fn wide_add(a: vec4<u32>, b: vec4<u32>) -> vec4<u32> {
    let low_a = half64(a.xy);
    let low = low_a + half64(b.xy);
    let carry = u64(select(0u, 1u, low < low_a));
    return words64(low, half64(a.zw) + half64(b.zw) + carry);
}
// Caller establishes a >= b, just as for the portable limb operation.
fn wide_sub(a: vec4<u32>, b: vec4<u32>) -> vec4<u32> {
    let low_a = half64(a.xy);
    let low_b = half64(b.xy);
    let borrow = u64(select(0u, 1u, low_a < low_b));
    return words64(low_a - low_b, half64(a.zw) - half64(b.zw) - borrow);
}
fn wide_less(a: vec4<u32>, b: vec4<u32>) -> bool {
    let high_a = half64(a.zw);
    let high_b = half64(b.zw);
    return high_a < high_b || (high_a == high_b && half64(a.xy) < half64(b.xy));
}
// These operations require a row certificate bounding every intermediate to
// u64; subtraction additionally requires a >= b. High wire words stay zero.
fn half_add(a: vec4<u32>, b: vec4<u32>) -> vec4<u32> {
    return words64(half64(a.xy) + half64(b.xy), u64(0u));
}
fn half_sub(a: vec4<u32>, b: vec4<u32>) -> vec4<u32> {
    return words64(half64(a.xy) - half64(b.xy), u64(0u));
}
fn half_less(a: vec4<u32>, b: vec4<u32>) -> bool {
    return half64(a.xy) < half64(b.xy);
}
