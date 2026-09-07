//! Emit the complete finite contract; output is evidence, not a benchmark.

use zetesis_gate_transfer_experiment::{Aliases, Domains, Operation, bitwise, lookup, reference};

fn main() {
    println!("operation\taliases\tdomains\treference\tbitwise\tlookup");
    for operation in Operation::ALL {
        for aliases in Aliases::ALL {
            for code in 0..64 {
                let domains = Domains::from_code(code).expect("enumerated six-bit domain");
                println!(
                    "{operation:?}\t{aliases:?}\t{code}\t{}\t{}\t{}",
                    reference(operation, aliases, domains).code(),
                    bitwise(operation, aliases, domains).code(),
                    lookup(operation, aliases, domains).code(),
                );
            }
        }
    }
}
