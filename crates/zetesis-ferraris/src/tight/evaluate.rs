use super::{
    TightCheck, TightCheckLimits, TightError, TightPlan, TightVerdict, Work, bytes, filled, reserve,
};
use crate::{Interpretation, Node};
use zetesis_cpu::{Control, Stop};

impl TightPlan {
    /// Check original satisfaction and every present atom's producer support.
    /// Ranked support discharges all proper-subset reduct queries at once.
    /// Failed support is returned as a residual, without claiming an original
    /// formula failure or publishing an unverified countermodel witness.
    ///
    /// # Errors
    /// Refuses foreign theory identities, cancellation, deadlines, allocation,
    /// and byte/work limits. An interrupted scan never returns stable.
    pub fn check(
        &self,
        candidate: &Interpretation,
        limits: TightCheckLimits,
        control: &Control,
    ) -> Result<TightCheck, TightError> {
        if !self.theory.same_instance(candidate.theory()) {
            return Err(Stop::WrongProgram.into());
        }
        control.poll()?;
        let logical_bytes = bytes(
            u128::from(self.statistics.resident_bytes)
                + self.theory.nodes().len() as u128
                + self.theory.atom_count() as u128,
            limits.max_bytes,
        )?;
        let mut work = Work {
            used: 0,
            max: limits.max_work,
            control,
        };
        // Byte cells, rather than implementation-dependent packed Vec<bool>,
        // give this primitive and future candidate tiles a clear payload bound.
        let mut values = reserve::<u8>(self.theory.nodes().len())?;
        for node in self.theory.nodes() {
            work.tick()?;
            values.push(u8::from(match *node {
                Node::Atom(atom) => candidate.contains(atom),
                Node::False => false,
                Node::And(a, b) => values[a] != 0 && values[b] != 0,
                Node::Or(a, b) => values[a] != 0 || values[b] != 0,
                Node::Implies(a, b) => values[a] == 0 || values[b] != 0,
            }));
        }
        for &root in self.theory.roots() {
            work.tick()?;
            if values[root] == 0 {
                return Ok(TightCheck {
                    verdict: TightVerdict::NotModel { root },
                    work: work.used,
                    logical_bytes,
                });
            }
        }
        let mut supported = filled(self.theory.atom_count(), 0u8)?;
        for producer in &self.producers {
            work.tick()?;
            if producer.body.is_none_or(|body| values[body] != 0) {
                supported[producer.head] = 1;
            }
        }
        for (atom, support) in supported.into_iter().enumerate() {
            work.tick()?;
            if candidate.contains(atom) && support == 0 {
                return Ok(TightCheck {
                    verdict: TightVerdict::Residual {
                        unsupported_atom: atom,
                    },
                    work: work.used,
                    logical_bytes,
                });
            }
        }
        Ok(TightCheck {
            verdict: TightVerdict::Stable,
            work: work.used,
            logical_bytes,
        })
    }
}
