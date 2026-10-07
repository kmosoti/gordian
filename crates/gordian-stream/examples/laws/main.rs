//! The learnable laws of the stream world, measured from the hidden side (W2; evaluator-side: the
//! tables read hidden labels, truth and rebuilt counterfactuals).
//!
//! ```text
//! cargo run -p gordian-stream --features reveal-hidden-state --example laws -- \
//!     --seed-from 40000 --count 200 --world a --out-dir artifacts/runs/w2/hidden/a-heldout [--owners]
//! ```
//!
//! For `--count` streams from `--seed-from` it writes five files into `--out-dir`:
//!
//! - `incidents.csv`: one row per incident: tier, family and mode, site and partner, instants,
//!   the incident it repeats, how many earlier incidents could have been repeated at its arrival,
//!   its decisive and out-of-catalogue message counts, the delay from the site's first alarm to
//!   the partner's, the regime epoch at its onset, and whether it was altered by the signature
//!   shift or by the added edge (rebuilt from its own plan under the first world's physics or the
//!   time-zero graph: `oracle::rebuilt_incident`) or merely exposed to the edge (a service newly
//!   downstream of its site or its partner).
//! - `regimes.csv`: the resolved regime changes.
//! - `streams.csv`: per stream counts.
//! - `vocab.csv`: free-form messages (public id at or above the catalogue limit) counted by id, the public cue (the number of abnormal counter readings, at most 3, at the message's service in the 10 s before it) and
//!   hidden class: the hard family an incident-borne message belongs to, or for a background
//!   message the hard family live anywhere, and at the message's service, at that instant.
//! - `owners.jsonl` (only with `--owners`): per stream, the incident each observation belongs to
//!   (`-1` for background), parallel to the delivered stream: the join key for a run's focus ids.
//!
//! `--world a` is the default parameters; `--world b` is recurrence 0.6 with the regime changes
//! at 150 s and 300 s (same kinds, same everything else). The output is a pure function of the
//! arguments, so two runs have the same sha256. The example needs the hidden-state feature;
//! without it the binary only says so. Nothing here reaches a policy.

#[cfg(not(feature = "reveal-hidden-state"))]
fn main() -> std::process::ExitCode {
    eprintln!("laws needs --features reveal-hidden-state");
    std::process::ExitCode::from(2)
}

#[cfg(feature = "reveal-hidden-state")]
fn main() -> std::process::ExitCode {
    real::main()
}

#[cfg(feature = "reveal-hidden-state")]
mod real {
    use gordian_stream::oracle::{
        EvidenceRole, NoiseKind, ObsLabel, RegimeDetail, StreamTruth, rebuilt_incident, reveal,
    };
    use gordian_stream::{
        ObsId, RegimeKind, RegimeSchedule, Stream, StreamParams, Tier, generate, params::SEC,
    };
    use gordian_world::graph::dependents_mask;
    use gordian_world::physics::{CATALOGUE_LIMIT, HIGH};
    use gordian_world::{FaultKind, Observation, Service, ServiceId};
    use std::collections::BTreeMap;
    use std::fmt::Write as _;
    use std::path::PathBuf;
    use std::process::ExitCode;

    fn usage() -> ExitCode {
        eprintln!("usage: laws [--seed-from N] [--count K] [--world a|b] --out-dir DIR [--owners]");
        ExitCode::from(2)
    }

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum World {
        A,
        B,
    }

    fn params(world: World, seed: u64) -> StreamParams {
        let mut p = StreamParams::new(seed);
        if world == World::B {
            p.recurrence_permille = 600;
            p.regimes = vec![
                RegimeSchedule {
                    at_ns: 150 * SEC,
                    change: RegimeKind::SignatureShift,
                },
                RegimeSchedule {
                    at_ns: 300 * SEC,
                    change: RegimeKind::EdgeAdd,
                },
            ];
        }
        p
    }

    fn kind_name(k: FaultKind) -> &'static str {
        match k {
            FaultKind::ResourceExhausted => "ResourceExhausted",
            FaultKind::ConfigDrift => "ConfigDrift",
            FaultKind::DependencyDown => "DependencyDown",
            FaultKind::CredentialExpired => "CredentialExpired",
            FaultKind::Intermittent => "Intermittent",
        }
    }

    fn family_name(h: gordian_stream::HardKind) -> &'static str {
        match h {
            gordian_stream::HardKind::Compound => "compound",
            gordian_stream::HardKind::Cascade => "cascade",
            gordian_stream::HardKind::SplitBrain => "split_brain",
            gordian_stream::HardKind::SlowLeak => "slow_leak",
        }
    }

    fn tier_name(t: Tier) -> &'static str {
        match t {
            Tier::Plain => "plain",
            Tier::Hard => "hard",
            Tier::Decoy => "decoy",
        }
    }

    /// The graph in force at `t`: the time-zero graph with every `EdgeAdd` at or before `t`.
    fn graph_at(truth: &StreamTruth, t: u64) -> Vec<Service> {
        let mut services = truth.services.clone();
        for r in &truth.regimes {
            if r.at_ns <= t
                && let RegimeDetail::EdgeAdd {
                    dependent,
                    dependency,
                } = r.detail
            {
                let dep = &mut services[dependent.index()].depends_on;
                dep.push(dependency);
                dep.sort();
            }
        }
        services
    }

    fn incomparable(services: &[Service], a: ServiceId, b: ServiceId) -> bool {
        a != b
            && !dependents_mask(services, a)[b.index()]
            && !dependents_mask(services, b)[a.index()]
    }

    fn newly_downstream(old: &[Service], new: &[Service], site: ServiceId) -> usize {
        let before = dependents_mask(old, site);
        let after = dependents_mask(new, site);
        before
            .iter()
            .zip(after.iter())
            .filter(|(b, a)| **a && !**b)
            .count()
    }

    fn is_abnormal_counter(obs: &Observation, service: ServiceId) -> bool {
        matches!(obs, Observation::Counter { service: s, value, .. } if *s == service && *value >= HIGH)
    }

    fn opt<T: std::fmt::Display>(x: Option<T>) -> String {
        x.map_or_else(String::new, |v| v.to_string())
    }

    pub fn main() -> ExitCode {
        let (mut seed_from, mut count) = (40_000u64, 200u64);
        let mut world = World::A;
        let mut out_dir: Option<PathBuf> = None;
        let mut owners = false;
        let mut args = std::env::args().skip(1);
        while let Some(flag) = args.next() {
            if flag == "--owners" {
                owners = true;
                continue;
            }
            let Some(value) = args.next() else {
                return usage();
            };
            match flag.as_str() {
                "--seed-from" => match value.parse() {
                    Ok(v) => seed_from = v,
                    Err(_) => return usage(),
                },
                "--count" => match value.parse() {
                    Ok(v) => count = v,
                    Err(_) => return usage(),
                },
                "--world" => match value.as_str() {
                    "a" => world = World::A,
                    "b" => world = World::B,
                    _ => return usage(),
                },
                "--out-dir" => out_dir = Some(PathBuf::from(value)),
                _ => return usage(),
            }
        }
        let Some(out_dir) = out_dir else {
            return usage();
        };
        if let Err(e) = std::fs::create_dir_all(&out_dir) {
            eprintln!("laws: cannot create {}: {e}", out_dir.display());
            return ExitCode::FAILURE;
        }

        let mut incidents_csv = String::from(
            "seed,incident,arrival,tier,family,mode,known_kind,duo,kind_a,kind_b,site,other,\
critical,onset_ns,deadline_ns,live_end_ns,busy_until_ns,recurrence_of,eligible_templates,\
epoch,n_obs,n_decisive,n_ext_msgs,site_first_alarm_ns,other_first_alarm_ns,other_is_dependent,\
sig_altered,edge_altered,edge_exposed\n",
        );
        let mut regimes_csv =
            String::from("seed,index,at_ns,kind,kind_a,kind_b,dependent,dependency,n_services\n");
        let mut streams_csv = String::from(
            "seed,world,services,incomparable_pairs,observations,freeform_observations,incidents,skipped_arrivals\n",
        );
        let mut vocab: BTreeMap<(u64, u64, String, String, String, u8), u64> = BTreeMap::new();
        let mut owners_jsonl = String::new();

        for seed in seed_from..seed_from.saturating_add(count) {
            let stream: Stream = generate(&params(world, seed));
            let truth = reveal(&stream);
            let events = stream.events();
            let n_free = events
                .iter()
                .filter(|(_, o)| matches!(o, Observation::Message { text_id, .. } if *text_id >= CATALOGUE_LIMIT))
                .count();
            let n_svc = truth.services.len();
            let pairs = (0..n_svc)
                .flat_map(|a| ((a + 1)..n_svc).map(move |b| (a, b)))
                .filter(|(a, b)| {
                    incomparable(&truth.services, ServiceId(*a as u32), ServiceId(*b as u32))
                })
                .count();
            writeln!(
                streams_csv,
                "{seed},{w},{},{pairs},{},{n_free},{},{}",
                truth.services.len(),
                events.len(),
                truth.incidents.len(),
                truth.skipped_arrivals,
                w = if world == World::A { "a" } else { "b" },
            )
            .unwrap();

            for (i, r) in truth.regimes.iter().enumerate() {
                let (kind, a, b, dependent, dependency) = match r.detail {
                    RegimeDetail::SignatureShift { kind, now_emits } => (
                        "signature_shift",
                        kind_name(kind).to_string(),
                        kind_name(now_emits).to_string(),
                        String::new(),
                        String::new(),
                    ),
                    RegimeDetail::EdgeAdd {
                        dependent,
                        dependency,
                    } => (
                        "edge_add",
                        String::new(),
                        String::new(),
                        dependent.0.to_string(),
                        dependency.0.to_string(),
                    ),
                    RegimeDetail::Nothing => (
                        "nothing",
                        String::new(),
                        String::new(),
                        String::new(),
                        String::new(),
                    ),
                };
                writeln!(
                    regimes_csv,
                    "{seed},{i},{},{kind},{a},{b},{dependent},{dependency},{}",
                    r.at_ns,
                    truth.services.len()
                )
                .unwrap();
            }

            let zero_graph = truth.services.clone();
            for inc in &truth.incidents {
                let onset = inc.onset_ns;
                let graph = graph_at(&truth, onset);
                let sh = &inc.shape;
                let site = inc.occupies[0];
                let other = sh.other;

                // Who could this arrival have repeated: earlier incidents whose services are all
                // free again at this onset, and for a cascade whose pair is still unconnected.
                let busy = |s: &ServiceId| {
                    truth.incidents[..inc.id as usize]
                        .iter()
                        .any(|k| k.occupies.contains(s) && k.busy_until_ns > onset)
                };
                let eligible = truth.incidents[..inc.id as usize]
                    .iter()
                    .filter(|j| {
                        j.occupies.iter().all(|s| !busy(s))
                            && match (j.shape.hard_kind, j.shape.other) {
                                (Some(gordian_stream::HardKind::Cascade), Some(p)) => {
                                    incomparable(&graph, j.occupies[0], p)
                                }
                                _ => true,
                            }
                    })
                    .count();

                let (family, mode) = match (inc.tier, sh.hard_kind) {
                    (Tier::Plain, _) => ("known", ""),
                    (_, Some(h)) => (
                        family_name(h),
                        match sh.contradicts_early {
                            Some(true) => "contradict",
                            Some(false) => "mimic",
                            None => "",
                        },
                    ),
                    (_, None) => ("unknown", ""),
                };
                let (kind_a, kind_b) = match (sh.known_kind, sh.pair, sh.hard_kind, sh.mimics) {
                    (Some(k), _, _, _) => (kind_name(k).to_string(), String::new()),
                    (_, Some((a, b)), _, _) => (kind_name(a).to_string(), kind_name(b).to_string()),
                    (_, _, Some(gordian_stream::HardKind::Cascade), Some(a)) => {
                        (kind_name(a).to_string(), String::new())
                    }
                    _ => (String::new(), String::new()),
                };

                let obs: Vec<(u64, &Observation)> = inc
                    .observations
                    .iter()
                    .map(|o: &ObsId| {
                        let (at, ob) = &events[o.0 as usize];
                        (at.0, ob)
                    })
                    .collect();
                let n_ext = inc
                    .decisive
                    .iter()
                    .filter(|o| {
                        matches!(&events[o.0 as usize].1, Observation::Message { text_id, .. } if *text_id >= CATALOGUE_LIMIT)
                    })
                    .count();
                let site_first = obs
                    .iter()
                    .find(|(_, ob)| is_abnormal_counter(ob, site))
                    .map(|(at, _)| *at);
                let other_first = other.and_then(|p| {
                    obs.iter()
                        .find(|(_, ob)| is_abnormal_counter(ob, p))
                        .map(|(at, _)| *at)
                });
                let other_is_dependent =
                    other.map(|p| dependents_mask(&graph, site)[p.index()] as u8);

                // Alteration by the regime changes, from the incident's own plan.
                let held: Vec<(u64, Observation)> = obs
                    .iter()
                    .map(|(at, ob)| (at - onset, (*ob).clone()))
                    .collect();
                let rebuilt =
                    rebuilt_incident(&stream, inc.id, false, false).expect("incident exists");
                if rebuilt != held {
                    eprintln!(
                        "laws: seed {seed} incident {}: the rebuilt incident differs from the stream's own",
                        inc.id
                    );
                    return ExitCode::FAILURE;
                }
                let base = rebuilt_incident(&stream, inc.id, true, false).expect("incident exists");
                let zero = rebuilt_incident(&stream, inc.id, false, true).expect("incident exists");
                let sig_altered = base != held;
                let edge_altered = zero != held;
                let exposed = newly_downstream(&zero_graph, &graph, site) > 0
                    || other.is_some_and(|p| {
                        inc.shape.hard_kind == Some(gordian_stream::HardKind::Cascade)
                            && newly_downstream(&zero_graph, &graph, p) > 0
                    });
                let epoch = truth.regimes.iter().filter(|r| r.at_ns <= onset).count();

                writeln!(
                    incidents_csv,
                    "{seed},{id},{arrival},{tier},{family},{mode},{known},{duo},{kind_a},{kind_b},{site},{other},{critical},{onset},{deadline},{live_end},{busy},{rec},{eligible},{epoch},{n_obs},{n_dec},{n_ext},{sfa},{ofa},{oid},{sa},{ea},{ex}",
                    id = inc.id,
                    arrival = inc.arrival,
                    tier = tier_name(inc.tier),
                    known = opt(sh.known_kind.map(kind_name)),
                    duo = if sh.known_kind.is_some() { (sh.duo as u8).to_string() } else { String::new() },
                    site = site.0,
                    other = opt(other.map(|p| p.0)),
                    critical = inc.critical as u8,
                    deadline = opt(inc.deadline_ns),
                    live_end = inc.live_end_ns,
                    busy = inc.busy_until_ns,
                    rec = opt(inc.recurrence_of),
                    n_obs = obs.len(),
                    n_dec = inc.decisive.len(),
                    sfa = opt(site_first),
                    ofa = opt(other_first),
                    oid = opt(other_is_dependent),
                    sa = sig_altered as u8,
                    ea = edge_altered as u8,
                    ex = exposed as u8,
                )
                .unwrap();
            }

            // Vocabulary: the public id of every free-form message, with the hidden class.
            let live_hard = |t: u64, service: Option<ServiceId>| -> String {
                truth
                    .incidents
                    .iter()
                    .find(|k| {
                        k.tier == Tier::Hard
                            && k.onset_ns <= t
                            && t <= k.live_end_ns
                            && service.is_none_or(|s| k.occupies.contains(&s))
                    })
                    .and_then(|k| k.shape.hard_kind)
                    .map_or("none", family_name)
                    .to_string()
            };
            // The public cue: how many abnormal counter readings (at or above the alarm level) the
            // message's service has had in the 10 s before it, capped at 3 (what an arm could
            // compute from the stream).
            let mut abnormal_at: Vec<Vec<u64>> = vec![Vec::new(); truth.services.len()];
            for (i, (at, ob)) in events.iter().enumerate() {
                if let Observation::Counter { service, value, .. } = ob
                    && *value >= HIGH
                {
                    abnormal_at[service.index()].push(at.0);
                }
                let Observation::Message {
                    service, text_id, ..
                } = ob
                else {
                    continue;
                };
                let cue = abnormal_at[service.index()]
                    .iter()
                    .rev()
                    .take_while(|t| at.0.saturating_sub(**t) <= 10 * SEC)
                    .count()
                    .min(3) as u8;
                if *text_id < CATALOGUE_LIMIT {
                    continue;
                }
                match truth.labels[i] {
                    ObsLabel::Incident { id, role } => {
                        let inc = &truth.incidents[id as usize];
                        let fam = inc.shape.hard_kind.map_or("none", family_name);
                        let class = match (inc.tier, role) {
                            (Tier::Hard, EvidenceRole::Decisive) => "incident",
                            _ => "incident_other",
                        };
                        *vocab
                            .entry((
                                seed,
                                *text_id,
                                class.to_string(),
                                fam.to_string(),
                                fam.to_string(),
                                cue,
                            ))
                            .or_insert(0) += 1;
                    }
                    ObsLabel::Background(kind) => {
                        let class = match kind {
                            NoiseKind::FreeForm => "background",
                            _ => "background_other",
                        };
                        let any = live_hard(at.0, None);
                        let here = live_hard(at.0, Some(*service));
                        *vocab
                            .entry((seed, *text_id, class.to_string(), any, here, cue))
                            .or_insert(0) += 1;
                    }
                }
            }

            if owners {
                let owner: Vec<String> = (0..events.len())
                    .map(|i| match truth.labels[i] {
                        ObsLabel::Incident { id, .. } => id.to_string(),
                        ObsLabel::Background(_) => "-1".to_string(),
                    })
                    .collect();
                writeln!(
                    owners_jsonl,
                    "{{\"seed\":{seed},\"owner\":[{}]}}",
                    owner.join(",")
                )
                .unwrap();
            }
        }

        let mut vocab_csv = String::from(
            "seed,text_id,class,any_live_hard_family,here_live_hard_family,cue,count\n",
        );
        for ((seed, id, class, any, here, cue), n) in &vocab {
            writeln!(vocab_csv, "{seed},{id},{class},{any},{here},{cue},{n}").unwrap();
        }

        let mut files = vec![
            ("incidents.csv", incidents_csv),
            ("regimes.csv", regimes_csv),
            ("streams.csv", streams_csv),
            ("vocab.csv", vocab_csv),
        ];
        if owners {
            files.push(("owners.jsonl", owners_jsonl));
        }
        for (name, body) in files {
            if let Err(e) = std::fs::write(out_dir.join(name), body) {
                eprintln!("laws: cannot write {name}: {e}");
                return ExitCode::FAILURE;
            }
        }
        ExitCode::SUCCESS
    }
}
