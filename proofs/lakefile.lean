import Lake
open Lake DSL

package zetesis where
  version := v!"0.4.0"

@[default_target]
lean_lib Zetesis where
  leanOptions := #[⟨`autoImplicit, false⟩]
