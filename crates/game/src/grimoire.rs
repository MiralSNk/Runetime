//! The grimoire: spell source, the last successful build and its report.

use crate::SCENARIO_A;
use crate::text::{cost, diagnostic, tr};
use lang::bytecode::Chunk;

/// Spell source and the result of the last build.
#[derive(Debug, Clone, PartialEq)]
pub struct Grimoire {
    /// Source text the player edits; starts with the example spell.
    pub source: String,
    /// The spell to cast: the first spell of the last successful build.
    pub spell: Option<Chunk>,
    /// Lines describing the last build.
    pub report: Vec<String>,
    /// Whether the last build failed.
    pub failed: bool,
}

impl Default for Grimoire {
    fn default() -> Self {
        Grimoire {
            source: SCENARIO_A.to_owned(),
            spell: None,
            report: vec![tr("grimoire.not_built", &[])],
            failed: false,
        }
    }
}

impl Grimoire {
    /// Compiles the source. On success the first spell becomes the one to
    /// cast; on failure the previous spell is dropped, so the arena never
    /// casts code that no longer matches the text.
    pub fn build(&mut self) {
        self.spell = None;
        self.failed = false;
        self.report = match lang::compile(&self.source) {
            Err(d) => {
                self.failed = true;
                vec![diagnostic(&d)]
            }
            Ok(chunks) => match chunks.into_iter().next() {
                None => vec![tr("grimoire.empty", &[])],
                Some(chunk) => {
                    let line = match vm::estimate(&chunk) {
                        Ok(e) => cost(e),
                        Err(e) => panic!("compiled code is corrupt: {e:?}"),
                    };
                    let report = vec![tr("grimoire.build.success", &[&chunk.name]), line];
                    self.spell = Some(chunk);
                    report
                }
            },
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SCENARIO_B;
    use vm::Estimate;

    #[test]
    fn builds_both_scenarios() {
        for src in [SCENARIO_A, SCENARIO_B] {
            let mut g = Grimoire {
                source: src.to_owned(),
                ..Default::default()
            };
            g.build();
            assert!(!g.failed && g.spell.is_some(), "{:?}", g.report);
        }
    }

    #[test]
    fn failed_build_drops_the_spell() {
        let mut g = Grimoire::default();
        g.build();
        g.source.push('$');
        g.build();
        assert!(g.failed && g.spell.is_none());
        assert!(g.report[0].starts_with("C001"));

        g.source = String::new();
        g.build();
        assert!(!g.failed && g.spell.is_none());
    }

    #[test]
    fn exact_cost_matches_the_cast() {
        let mut g = Grimoire::default();
        g.build();
        let spell = g.spell.unwrap();
        let outcome = crate::arena::Arena::default().cast(&spell);
        assert_eq!(vm::estimate(&spell), Ok(Estimate::Exact(outcome.fuel_used)));
    }
}
