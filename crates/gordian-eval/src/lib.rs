//! The evaluator: scores a recorded trajectory against the hidden truth of its episode.
//!
//! Built (A2 in `docs/local-test-plan.md`): [`Truth`], [`Step`], [`Verdict`], [`EvalError`] and
//! [`score`]. The rules are numbered in `RULES.md`; hand-written cases that pin each rule are in
//! `fixtures/tiny-cases.json`. Deviations from the plan's sketch are listed in `RULES.md`.
//!
//! This crate depends on `gordian-core` and `gordian-world` (with `reveal-hidden-state`) and on
//! no policy or component crate, and `score` takes a [`Truth`] rather than an `Episode`, so
//! fixtures need not run the generator. [`Truth::from_episode`] is the only call into the oracle.

#![forbid(unsafe_code)]

mod score;
mod timeserde;
mod truth;

pub use score::{EvalError, Step, Verdict, score};
pub use truth::Truth;
