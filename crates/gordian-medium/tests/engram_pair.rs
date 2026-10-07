//! Keys over a pair of nodes (work item A1c): the cells of a pair engram, a recall that needs its
//! features at two nodes and at their edge, the fired pair, comparison by role, generalisation that
//! keeps the span, and persistence. `DESIGN.md`, "The two-site key and the recall gate (A1c)".

use gordian_medium::engram::{pair_bytes, restore_pair};
use gordian_medium::{
    Address, BindResult, CollectingEffector, ConstantField, CountingLedger, EngramParams, Engrams,
    Event, FeatureRole, Field, Key, KeySite, Limits, Medium, NoTrace, Outcome, OutcomeSite, Ports,
    Prices, Recall, ScriptedSense, StepClock, Tag, pair_node, pair_of,
};

const TICK: u64 = 100_000_000;
const DECAY_PERIOD: u64 = 10_000_000_000;
const KIND: u16 = 7;

fn params(generalise: bool) -> EngramParams {
    EngramParams {
        domain: 0,
        window_ticks: 5,
        min_features: 2,
        gain: 1.0,
        max_strength: 4.0,
        threshold: 0.5,
        penalty: 1.0,
        decay: 0.5,
        decay_rhythm: 0,
        refractory_ticks: 10,
        generalise,
        kind: KIND,
    }
}

fn fresh(generalise: bool) -> (Medium, Engrams) {
    let spec = Engrams::medium_spec(TICK, DECAY_PERIOD, Limits::default(), Prices::DECLARED);
    (
        Medium::from_spec(&spec).unwrap(),
        Engrams::new(params(generalise), &[0, 1, 2]).unwrap(),
    )
}

fn ev(tick: u64, node: u16, tag: u32, seq: u32) -> Event {
    Event {
        tick,
        offset_ns: 0,
        source: Address {
            domain: 0,
            node,
            channel: 0,
        },
        tags: vec![Tag(tag)],
        value: 1.0,
        seq,
    }
}

/// Run ticks `from..to`, resolving each tick's recalls with `recall_in` after the tick.
fn run(
    m: &mut Medium,
    e: &mut Engrams,
    from: u64,
    to: u64,
    events: Vec<Event>,
) -> Vec<(u64, Recall)> {
    let mut sense = ScriptedSense::from_events(events);
    let mut field = ConstantField(Field::default());
    let mut ledger = CountingLedger::default();
    let mut trace = NoTrace;
    let mut out = Vec::new();
    for t in from..to {
        let mut effector = CollectingEffector::default();
        let mut clock = StepClock::new(t, TICK);
        let mut ports = Ports {
            clock: &mut clock,
            sense: &mut sense,
            field: &mut field,
            effector: &mut effector,
            ledger: &mut ledger,
            trace: &mut trace,
            plasticity: e,
        };
        m.step(&mut ports).unwrap();
        for (_, p) in &effector.proposals {
            if let Some(r) = e.recall_in(m, p) {
                out.push((t, r));
            }
        }
    }
    out
}

const SITE: FeatureRole = FeatureRole::Site;
const PARTNER: FeatureRole = FeatureRole::Partner;
const REL: FeatureRole = FeatureRole::Relation;

fn pair_key(f: &[(u32, FeatureRole)]) -> Key {
    Key::with_roles(f.iter().map(|(t, r)| (Tag(*t), *r, false)), KeySite::Pair)
}

const OUT_A: Outcome = Outcome {
    tag: Tag(1),
    site: OutcomeSite::Support,
};
const OUT_B: Outcome = Outcome {
    tag: Tag(2),
    site: OutcomeSite::Support,
};

#[test]
fn edge_nodes_name_ordered_pairs_and_the_store_refuses_nodes_without_one() {
    assert_eq!(pair_node(3, 5), 0x8000 | 3 << 7 | 5);
    assert_eq!(pair_of(pair_node(3, 5)), Some((3, 5)));
    assert_eq!(pair_of(pair_node(5, 3)), Some((5, 3)));
    assert_eq!(pair_of(11), None);
    assert!(Engrams::new(params(false), &[0, 127]).is_ok());
    assert!(Engrams::new(params(false), &[0, 128]).is_err());
}

#[test]
fn a_pair_engram_has_a_key_cell_per_ordered_pair_wired_by_role() {
    let (mut m, mut e) = fresh(false);
    let b = e.bind(
        &mut m,
        &pair_key(&[(50, REL), (10, SITE), (20, PARTNER)]),
        OUT_A,
    );
    assert_eq!(b.result, BindResult::Created(0));
    let g = &e.engrams()[0];
    // Six ordered pairs of three nodes, first node major.
    let pairs: Vec<(u16, u16)> = g
        .coincidences
        .iter()
        .map(|c| pair_of(c.0).unwrap())
        .collect();
    assert_eq!(pairs, [(0, 1), (0, 2), (1, 0), (1, 2), (2, 0), (2, 1)]);
    // Sense cells: the relation at six edge nodes, the site and partner features at three nodes
    // each; then six key cells, a latch, an emitter.
    assert_eq!(m.cells().len(), 6 + 3 + 3 + 6 + 2);
    assert_eq!(m.synapses().len(), 6 * 3 + 6 + 1);
    // Key cell (1, 2): relation at pair_node(1, 2), site feature at 1, partner feature at 2.
    let c = g
        .coincidences
        .iter()
        .position(|c| c.0 == pair_node(1, 2))
        .unwrap();
    let k = g.key.features().len();
    let from: Vec<_> = (0..k)
        .map(|f| {
            let s = &m.synapses()[g.inputs[c * k + f].0 as usize];
            m.cells()[s.from.0 as usize].pattern.unwrap().node.unwrap()
        })
        .collect();
    assert_eq!(from, [pair_node(1, 2), 1, 2]);
}

#[test]
fn a_pair_recall_needs_the_site_the_partner_and_their_edge_and_names_the_pair() {
    let (mut m, mut e) = fresh(false);
    e.bind(
        &mut m,
        &pair_key(&[(50, REL), (10, SITE), (20, PARTNER)]),
        OUT_A,
    );
    // Site feature at 2, partner feature at 0, the relation of 2 to 0: the pair (2, 0) fires.
    let r = run(
        &mut m,
        &mut e,
        0,
        10,
        vec![
            ev(2, 0, 20, 0),
            ev(3, 2, 10, 1),
            ev(3, pair_node(2, 0), 50, 2),
        ],
    );
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].0, 3);
    assert_eq!(r[0].1.fired, Some(pair_node(2, 0)));
    assert_eq!(
        r[0].1.anchor.seq, 0,
        "the anchor is the earliest event, the partner's here"
    );

    // Missing the relation: nothing. The relation of another pair: nothing.
    let (mut m, mut e) = fresh(false);
    e.bind(
        &mut m,
        &pair_key(&[(50, REL), (10, SITE), (20, PARTNER)]),
        OUT_A,
    );
    assert!(
        run(
            &mut m,
            &mut e,
            0,
            10,
            vec![ev(2, 0, 20, 0), ev(3, 2, 10, 1)]
        )
        .is_empty()
    );
    assert!(
        run(
            &mut m,
            &mut e,
            10,
            20,
            vec![
                ev(12, 0, 20, 0),
                ev(13, 2, 10, 1),
                ev(13, pair_node(2, 1), 50, 2)
            ]
        )
        .is_empty()
    );
    // The features with their roles swapped (site feature at the partner): nothing.
    assert!(
        run(
            &mut m,
            &mut e,
            20,
            30,
            vec![
                ev(22, 0, 10, 0),
                ev(23, 2, 20, 1),
                ev(23, pair_node(2, 0), 50, 2)
            ]
        )
        .is_empty()
    );
    // Both features at one node with the edge of a pair through it: nothing (the partner is
    // another node).
    assert!(
        run(
            &mut m,
            &mut e,
            30,
            40,
            vec![
                ev(32, 2, 20, 0),
                ev(33, 2, 10, 1),
                ev(33, pair_node(2, 0), 50, 2)
            ]
        )
        .is_empty()
    );
}

#[test]
fn a_one_site_recall_resolved_in_the_medium_names_its_node() {
    let (mut m, mut e) = fresh(false);
    e.bind(
        &mut m,
        &Key::new([Tag(10), Tag(11)], KeySite::Variable),
        OUT_A,
    );
    let r = run(
        &mut m,
        &mut e,
        0,
        10,
        vec![ev(2, 1, 10, 0), ev(3, 1, 11, 1)],
    );
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].1.fired, Some(1));
}

#[test]
fn features_compare_by_role_in_contradiction_and_exact_match() {
    let (mut m, mut e) = fresh(false);
    // A one-site engram keyed {10, 11}.
    e.bind(
        &mut m,
        &Key::new([Tag(10), Tag(11)], KeySite::Variable),
        OUT_A,
    );
    // A pair pattern with 10 and 11 at its site disagrees: the one-site engram would recall on
    // it at the pair's first node, so it is contradicted.
    let b = e.bind(
        &mut m,
        &pair_key(&[(50, REL), (10, SITE), (11, SITE), (20, PARTNER)]),
        OUT_B,
    );
    assert_eq!(b.contradicted, vec![0]);
    // With 11 at the partner only, it is not.
    let b = e.bind(
        &mut m,
        &pair_key(&[(50, REL), (10, SITE), (11, PARTNER)]),
        OUT_B,
    );
    assert!(b.contradicted.is_empty());
    // A one-site pattern never contradicts a pair engram (it holds no relation).
    let b = e.bind(
        &mut m,
        &Key::new([Tag(10), Tag(11), Tag(20), Tag(50)], KeySite::Variable),
        OUT_A,
    );
    assert!(b.contradicted.is_empty());
    // The same tags with other roles are another key.
    let (mut m, mut e) = fresh(false);
    e.bind(
        &mut m,
        &pair_key(&[(50, REL), (10, SITE), (20, PARTNER)]),
        OUT_A,
    );
    let b = e.bind(
        &mut m,
        &pair_key(&[(50, REL), (20, SITE), (10, PARTNER)]),
        OUT_A,
    );
    assert_eq!(b.result, BindResult::Created(1));
    let b = e.bind(
        &mut m,
        &pair_key(&[(20, PARTNER), (10, SITE), (50, REL)]),
        OUT_A,
    );
    assert_eq!(
        b.result,
        BindResult::Strengthened(0),
        "order does not matter"
    );
}

#[test]
fn generalisation_keeps_a_pair_engram_spanning_two_nodes() {
    let (mut m, mut e) = fresh(true);
    e.bind(
        &mut m,
        &pair_key(&[(50, REL), (10, SITE), (11, SITE), (20, PARTNER)]),
        OUT_A,
    );
    // Shares the relation, a site and the partner feature: narrowed to those three.
    let b = e.bind(
        &mut m,
        &pair_key(&[(50, REL), (10, SITE), (12, SITE), (20, PARTNER)]),
        OUT_A,
    );
    assert_eq!(b.result, BindResult::Generalised(0));
    assert_eq!(
        e.engrams()[0].live_role_tags(),
        [(REL, Tag(50)), (SITE, Tag(10)), (PARTNER, Tag(20))]
    );
    // Shares two site features but no partner feature: not narrowed, a new engram.
    let (mut m, mut e) = fresh(true);
    e.bind(
        &mut m,
        &pair_key(&[(50, REL), (10, SITE), (11, SITE), (20, PARTNER)]),
        OUT_A,
    );
    let b = e.bind(
        &mut m,
        &pair_key(&[(50, REL), (10, SITE), (11, SITE), (21, PARTNER)]),
        OUT_A,
    );
    assert_eq!(b.result, BindResult::Created(1));
    // Another relation (order or gap band): not narrowed.
    let b = e.bind(
        &mut m,
        &pair_key(&[(51, REL), (10, SITE), (11, SITE), (20, PARTNER)]),
        OUT_A,
    );
    assert_eq!(b.result, BindResult::Created(2));
    // A one-site key is never narrowed into a pair engram, nor a pair key into a one-site one.
    let b = e.bind(
        &mut m,
        &Key::new([Tag(10), Tag(11)], KeySite::Variable),
        OUT_A,
    );
    assert_eq!(b.result, BindResult::Created(3));
}

#[test]
fn a_table_with_pair_engrams_persists_restores_and_refuses_a_bad_role() {
    let (mut m, mut e) = fresh(true);
    e.bind(
        &mut m,
        &Key::new([Tag(10), Tag(11)], KeySite::Variable),
        OUT_A,
    );
    e.bind(
        &mut m,
        &pair_key(&[(50, REL), (10, SITE), (20, PARTNER)]),
        OUT_B,
    );
    let bytes = pair_bytes(&m, &e);
    let (m2, e2) = restore_pair(&bytes).unwrap();
    assert_eq!(pair_bytes(&m2, &e2), bytes);
    assert_eq!(e2.engrams()[1].key.roles(), &[REL, SITE, PARTNER]);
    // Every single-byte change to the table is refused or re-encodes to itself.
    let table = e.to_bytes();
    for i in 0..table.len() {
        let mut t = table.clone();
        t[i] ^= 0x01;
        if let Ok(x) = Engrams::from_bytes(&t, &m) {
            assert_eq!(x.to_bytes(), t);
        }
    }
    // A one-site table's bytes carry no roles: the engram's bytes are A1a's.
    let (mut m3, mut e3) = fresh(true);
    e3.bind(
        &mut m3,
        &Key::new([Tag(10), Tag(11)], KeySite::Variable),
        OUT_A,
    );
    assert!(table.starts_with(&e3.to_bytes()[..5]));
}
