import Zetesis.StructuralBindings

/-!
# Finite integer envelopes with retained comparisons

Directed order paths and affine bounds provide finite supersets of the values
satisfying a source guard. Strict comparisons and correlated variables remain
in that whole guard: a Cartesian envelope does not assert independence of its
coordinates. Equality supplies both order directions; an undirected connection
alone supplies neither a lower nor an upper bound.

The laws concern mathematical integers and supplied sound comparisons. They do
not recognize safe source expressions, normalize affine syntax, bound analysis
work, or prove Rust checked arithmetic and cursor refinement. In particular,
the divisions below are mathematical floor division by a positive integer,
not the source language's truncating division operation. Runtime inference must
establish that its endpoint rounding implements these bounds.
-/

namespace Zetesis.IntegerEnvelopes

universe u v
variable {V : Type u} {B : Type v}

/-- A directed comparison may carry an integer separation, such as one for `<`.
Its orientation always reads `value lower + separation ≤ value upper`. -/
structure Comparison (V : Type u) where
  lower : V
  upper : V
  separation : Int

/-- The interpreted upper vertex respects this directed lower separation. -/
def Satisfied (value : V → Int) (edge : Comparison V) : Prop :=
  value edge.lower + edge.separation ≤ value edge.upper

/-- Finite paths accumulate the separation imposed by their comparisons. -/
inductive Path (edges : List (Comparison V)) : V → V → Int → Prop
  | same (vertex : V) : Path edges vertex vertex 0
  | next (edge : Comparison V) (present : edge ∈ edges)
      {last : V} {distance : Int} (rest : Path edges edge.upper last distance) :
      Path edges edge.lower last (edge.separation + distance)

/-- Sound comparisons imply the accumulated bound of every finite path.
Induction composes the first comparison with the remaining path's bound. -/
theorem path_bound (edges : List (Comparison V)) (value : V → Int)
    (sound : ∀ edge ∈ edges, Satisfied value edge)
    {first last : V} {distance : Int} (path : Path edges first last distance) :
    value first + distance ≤ value last := by
  induction path with
  | same => omega
  | next edge present _ ih =>
    have step : value edge.lower + edge.separation ≤ value edge.upper := sound edge present
    omega

/-- Both directed anchors are needed for a finite envelope. -/
theorem anchored_envelope (edges : List (Comparison V)) (value : V → Int)
    (sound : ∀ edge ∈ edges, Satisfied value edge)
    (lower upper vertex : V) (below above : Int)
    (lower_path : Path edges lower vertex below)
    (upper_path : Path edges vertex upper above) :
    value lower + below ≤ value vertex ∧ value vertex ≤ value upper - above := by
  have lower_bound := path_bound edges value sound lower_path
  have upper_bound := path_bound edges value sound upper_path
  omega

/-- Interval addition is sound even when its two operands are correlated. -/
theorem addition_envelope (x y lowerX upperX lowerY upperY : Int)
    (left : lowerX ≤ x ∧ x ≤ upperX) (right : lowerY ≤ y ∧ y ≤ upperY) :
    lowerX + lowerY ≤ x + y ∧ x + y ≤ upperX + upperY := by
  omega

/-- Negation reverses endpoint order; it does not discard a negative coefficient. -/
theorem negation_envelope (x lower upper : Int) (inside : lower ≤ x ∧ x ≤ upper) :
    -upper ≤ -x ∧ -x ≤ -lower := by
  omega

/-- A coefficient's sign determines which operand endpoint bounds its product
from below. Negative coefficients reverse the order comparison. -/
theorem product_lower_bound (coefficient x lower upper : Int)
    (inside : lower ≤ x ∧ x ≤ upper) :
    (if 0 ≤ coefficient then coefficient * lower else coefficient * upper) ≤
      coefficient * x := by
  by_cases nonnegative : 0 ≤ coefficient
  · simpa [nonnegative] using Int.mul_le_mul_of_nonneg_left inside.1 nonnegative
  · have nonpositive : coefficient ≤ 0 := by omega
    simpa [nonnegative] using Int.mul_le_mul_of_nonpos_left nonpositive inside.2

/-- Lower bounds compose through a finite linear sum even when variables repeat
or their values are correlated. The proof adds one valid term bound at a time. -/
theorem sum_lower_bound (terms : List V) (value lower : V → Int)
    (bounded : ∀ term ∈ terms, lower term ≤ value term) :
    (terms.map lower).sum ≤ (terms.map value).sum := by
  induction terms with
  | nil => simp
  | cons term rest ih =>
    have first := bounded term (List.mem_cons_self ..)
    have remaining := ih (fun item member => bounded item (List.mem_cons_of_mem term member))
    simp only [List.map_cons, List.sum_cons]
    omega

/-- Replacing the other summands by their lower bound preserves every solution
of an upper comparison; positive division then rounds the target downwards. -/
theorem positive_endpoint (coefficient x remainder bound lowerRest : Int)
    (positive : 0 < coefficient) (rest : lowerRest ≤ remainder)
    (comparison : coefficient * x + remainder ≤ bound) :
    x ≤ (bound - lowerRest) / coefficient := by
  rw [Int.le_ediv_iff_mul_le positive]
  have product : coefficient * x = x * coefficient := Int.mul_comm _ _
  omega

/-- A negative coefficient gives a lower target endpoint. Reflecting the target
reduces the argument to positive floor division, hence this negative quotient
is the required ceiling rather than truncation toward zero. -/
theorem negative_endpoint (coefficient x remainder bound lowerRest : Int)
    (negative : coefficient < 0) (rest : lowerRest ≤ remainder)
    (comparison : coefficient * x + remainder ≤ bound) :
    -((bound - lowerRest) / (-coefficient)) ≤ x := by
  have reflected : -x ≤ (bound - lowerRest) / (-coefficient) := by
    rw [Int.le_ediv_iff_mul_le (by omega : 0 < -coefficient), Int.neg_mul_neg]
    have product : coefficient * x = x * coefficient := Int.mul_comm _ _
    omega
  omega

/-- Invert a positive affine coefficient after enclosing the other terms.
The endpoints round outwards, including for negative numerators. -/
theorem affine_envelope (coefficient x remainder lower upper lowerRest upperRest : Int)
    (positive : 0 < coefficient)
    (rest : lowerRest ≤ remainder ∧ remainder ≤ upperRest)
    (bounded : lower ≤ coefficient * x + remainder ∧
      coefficient * x + remainder ≤ upper) :
    (lower - upperRest + coefficient - 1) / coefficient ≤ x ∧
      x ≤ (upper - lowerRest) / coefficient := by
  constructor
  · rw [Int.ediv_le_iff_le_mul positive]
    have product : coefficient * x = x * coefficient := Int.mul_comm _ _
    omega
  · rw [Int.le_ediv_iff_mul_le positive]
    have product : coefficient * x = x * coefficient := Int.mul_comm _ _
    omega

/-- A closed integer envelope is finite, including when it is empty. -/
def integers (lower upper : Int) : List Int :=
  (List.range (upper - lower + 1).toNat).map (fun offset : Nat => lower + (offset : Int))

/-- Enumeration contains exactly the integers between both inclusive endpoints,
and contains none when the lower endpoint exceeds the upper endpoint. -/
theorem integer_membership (lower upper value : Int) :
    value ∈ integers lower upper ↔ lower ≤ value ∧ value ≤ upper := by
  simp only [integers, List.mem_map, List.mem_range]
  constructor
  · rintro ⟨offset, below, rfl⟩
    omega
  · intro inside
    refine ⟨(value - lower).toNat, ?_, ?_⟩ <;> omega

/-- Complete envelope enumeration followed by the original guard recovers exact
bindings. Coverage may be conservative; the guard itself is never removed. -/
theorem filtered_bindings_exact (candidates : List B) (guard : B → Bool)
    (coverage : ∀ binding, guard binding = true → binding ∈ candidates) (binding : B) :
    binding ∈ candidates.filter guard ↔ guard binding = true := by
  rw [List.mem_filter]
  exact ⟨And.right, fun accepted => ⟨coverage binding accepted, accepted⟩⟩

end Zetesis.IntegerEnvelopes
