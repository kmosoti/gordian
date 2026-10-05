//! The order in which the arms of an interleaved run play one episode (plan A8).
//!
//! Wall time on this VM drifts within a session and differs between sessions, and the first run
//! of an episode in a process may differ from the second (caches, allocator, branch predictors).
//! Arms compared on measured cost must therefore share the machine's state: every `(seed, class)`
//! is played once per arm, back to back, in an order drawn per episode, so that no arm is
//! systematically first and a slow stretch of the machine falls on every arm alike. The draw
//! does not spread a position effect away; it makes the arm's position independent of the arm,
//! so the effect is symmetric noise and not bias, and `analysis/gordian_analysis/drift.py` tests
//! whether there is one.
//!
//! The order is a pure function of `(run_seed, seed, class, number of arms)`: a ChaCha8 stream
//! seeded from the three, shuffled with Fisher-Yates. It does not read the clock, a result, a
//! cost or the arms' names, and it is drawn before any arm plays the episode.

use crate::recorder::class_hash;
use gordian_world::EpisodeClass;
use rand_chacha::ChaCha8Rng;
use rand_core::{Rng, SeedableRng};

/// Separates this stream from every other use of the same three numbers (the random arm's
/// generator is seeded from the episode seed and the arm name, a different input).
const DOMAIN: u64 = 0x6172_6d2d_6f72_6465; // "arm-orde"

fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// The 32-byte generator seed for `(run_seed, seed, class)`, expanded with splitmix64 rather
/// than a library's seed expansion, so that no dependency's choice is a hidden input.
pub fn order_seed(run_seed: u64, seed: u64, class: EpisodeClass) -> [u8; 32] {
    order_seed_keyed(run_seed, seed, class_hash(class))
}

/// [`order_seed`] with the class replaced by any 64-bit key. A world with no episode classes (the
/// stream harness) passes its own constant, so that its draws are a different stream from every
/// episode's and the same function of `(run_seed, seed, key)`.
pub fn order_seed_keyed(run_seed: u64, seed: u64, key: u64) -> [u8; 32] {
    let mut state = DOMAIN;
    let mut folded = 0u64;
    for word in [run_seed, seed, key] {
        state ^= word;
        folded ^= splitmix64(&mut state);
    }
    let mut state = folded;
    let mut bytes = [0u8; 32];
    for chunk in bytes.as_chunks_mut::<8>().0 {
        *chunk = splitmix64(&mut state).to_le_bytes();
    }
    bytes
}

/// The order in which the arms play `(seed, class)`: a permutation of `0..arms`, where
/// `order[p]` is the index of the arm at position `p`. One arm gives `[0]`.
pub fn arm_order(run_seed: u64, seed: u64, class: EpisodeClass, arms: usize) -> Vec<usize> {
    arm_order_keyed(run_seed, seed, class_hash(class), arms)
}

/// [`arm_order`] with the class replaced by any 64-bit key (see [`order_seed_keyed`]).
pub fn arm_order_keyed(run_seed: u64, seed: u64, key: u64, arms: usize) -> Vec<usize> {
    let mut order: Vec<usize> = (0..arms).collect();
    let mut rng = ChaCha8Rng::from_seed(order_seed_keyed(run_seed, seed, key));
    // Fisher-Yates. `(r * (i + 1)) >> 64` maps a uniform u64 to `0..=i` with a bias of at most
    // `(i + 1) / 2^64`, which no experiment here can see.
    for i in (1..order.len()).rev() {
        let r = u128::from(rng.next_u64());
        let j = ((r * (i as u128 + 1)) >> 64) as usize;
        order.swap(i, j);
    }
    order
}

/// The position of each arm: the inverse of [`arm_order`], so that `positions[a]` is where arm
/// `a` plays.
pub fn positions(order: &[usize]) -> Vec<usize> {
    let mut positions = vec![0; order.len()];
    for (position, arm) in order.iter().enumerate() {
        positions[*arm] = position;
    }
    positions
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_order_is_a_permutation_and_a_function_of_its_inputs() {
        for arms in 1..=6 {
            for seed in 0..50 {
                let order = arm_order(7, seed, EpisodeClass::Ambiguous, arms);
                let mut sorted = order.clone();
                sorted.sort_unstable();
                assert_eq!(sorted, (0..arms).collect::<Vec<_>>());
                assert_eq!(order, arm_order(7, seed, EpisodeClass::Ambiguous, arms));
                let inverse = positions(&order);
                for (position, arm) in order.iter().enumerate() {
                    assert_eq!(inverse[*arm], position);
                }
            }
        }
    }

    #[test]
    fn one_arm_is_always_first() {
        assert_eq!(arm_order(1, 2, EpisodeClass::NoFault, 1), vec![0]);
    }

    #[test]
    fn every_input_changes_the_stream() {
        let base = order_seed(1, 2, EpisodeClass::Ambiguous);
        assert_ne!(base, order_seed(2, 2, EpisodeClass::Ambiguous));
        assert_ne!(base, order_seed(1, 3, EpisodeClass::Ambiguous));
        assert_ne!(base, order_seed(1, 2, EpisodeClass::NoFault));
        // Swapping run_seed and seed must not give the same stream.
        assert_ne!(
            order_seed(1, 2, EpisodeClass::Ambiguous),
            order_seed(2, 1, EpisodeClass::Ambiguous)
        );
    }

    #[test]
    fn two_arms_each_go_first_about_half_the_time() {
        let n = 4000u64;
        let first = (0..n)
            .filter(|seed| arm_order(11, *seed, EpisodeClass::Duplicates, 2)[0] == 0)
            .count() as f64;
        // Binomial(4000, 1/2): sd 31.6; 5 sd is 158.
        assert!((first - 2000.0).abs() < 158.0, "arm 0 first {first} of {n}");
    }

    #[test]
    fn three_arms_reach_all_six_orders_evenly() {
        use std::collections::BTreeMap;
        let n = 6000u64;
        let mut counts: BTreeMap<Vec<usize>, u32> = BTreeMap::new();
        for seed in 0..n {
            *counts
                .entry(arm_order(3, seed, EpisodeClass::NoiseFlood, 3))
                .or_default() += 1;
        }
        assert_eq!(counts.len(), 6);
        // Binomial(6000, 1/6): sd 28.9; 5 sd is 145.
        for (order, count) in counts {
            assert!(
                (f64::from(count) - 1000.0).abs() < 145.0,
                "{order:?}: {count}"
            );
        }
    }
}
