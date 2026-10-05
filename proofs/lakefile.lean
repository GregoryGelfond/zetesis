import Lake
open Lake DSL

package zetesis where
  version := v!"0.3.0"

@[default_target]
lean_lib Zetesis where
  leanOptions := #[⟨`autoImplicit, false⟩]
