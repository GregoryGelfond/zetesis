// The syntax error must point into this invocation, not the facade wrapper.
// Keep the consumer token span in the reviewed diagnostic snapshot.
fn main() {
    let _ = z::fact!(p(1 2));
}
