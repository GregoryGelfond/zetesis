# Constraints specialized by fixed witnesses

A fixed fact relation can supply several witnesses for the same remaining
constraint. For example, in `:- link(N,M,V), valve(V), node(M), bad(N,T).`, the
fixed block may have many admissible `(M,V)` pairs for one `N`. If these are its
only producers, the complete block can be projected to the distinct values of
`N`, leaving one constraint `:- bad(n,T).` for each projected value.

[`FixedWitnessConstraints.bodies_fire_together`](../Zetesis/FixedWitnessConstraints.lean)
separates that argument from the compiler. Its finite witness list contains the
complete admissible fixed block; its key list contains exactly their projected
keys. The residual body is a function of the key, so it cannot inspect eliminated
coordinates. Each witness condition must be true in the interpretation at issue.
The law proves that the original and projected bodies fire together. Duplicates
in either list do not matter. An empty witness list yields no keys, whereas a
nonempty block with no remaining coordinates yields the one empty tuple.

For answer-set preservation, establish these premises on every answer set of the
unchanged rest of the program. Then apply
[`KeyedConstraints.asked_constraints_preserve`](../Zetesis/KeyedConstraints.lean)
to the two finite ground constraint families. The surrounding program and atom
vocabulary stay the same, and the final reduct definition is unchanged. The law
does not authorize dropping a condition that is merely possible, or projecting
each coordinate independently and inventing tuple combinations.

Source use still requires complete all-producer certification, exact correlated
tuples, proper scope and typed substitution, and a faithful mapping from each
original constraint to its replacement family. Partial analysis cannot certify
an empty block. Arithmetic reachability, constructor limits, provenance,
cancellation, allocation and charged resource behavior remain compiler
obligations. This law does not verify the Rust recognizer or promise an execution
improvement.
