//! The general incremental core: ordered relations, deltas, a projection operator, counted
//! operations and the declared price table.
//!
//! Nothing here names an observation, an anomaly or a counter. A [`Table`] is an ordered relation
//! (a `BTreeMap`: every iteration is in key order, so no output depends on hashing or insertion
//! history); a logged table records a [`Delta`] for every change, which a consumer drains;
//! [`project`] keeps a second table equal to a projection or a filter of the first, from the first's
//! deltas alone (an incremental map: a row is touched only when its projection changed).
//!
//! # What is counted
//!
//! Every table counts its own probes (point reads), writes (a row put or removed) and scanned rows
//! (one per row a range scan yields), with `Cell` counters, so a read through `&self` counts. The
//! program counts fires (one per rule or operator invocation) itself. [`Table::peek`] and
//! [`Table::scan_free`] read without counting: they exist for the seam adapter (the view the
//! rung reads), whose work is not part of the modelled cost (see the module documentation of the
//! parent), and nothing in a rule uses them.
//!
//! # The price table
//!
//! [`Prices::DECLARED`] is a table of nanoseconds per counted operation, calibrated by the
//! criterion benchmark `bench.rs`: workloads that isolate each kind (probe, write, scan, fire), and
//! workloads that run the whole program over generated scenarios, whose measured time per counted
//! operation includes everything around the table operations. The report of C1 gives the measured
//! values beside the declared ones. It prices this reference implementation on this machine, as the
//! medium's prices do theirs.

use std::cell::Cell;
use std::collections::BTreeMap;
use std::collections::btree_map::Range;
use std::ops::RangeBounds;

/// Counted operations: what the engine did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct OpCounts {
    /// Point reads of a table.
    pub probes: u64,
    /// Rows put or removed (a logged table's delta included).
    pub writes: u64,
    /// Rows visited by range scans.
    pub scans: u64,
    /// Rule or operator invocations.
    pub fires: u64,
}

impl OpCounts {
    /// The sum of two counts.
    pub fn plus(self, other: Self) -> Self {
        Self {
            probes: self.probes + other.probes,
            writes: self.writes + other.writes,
            scans: self.scans + other.scans,
            fires: self.fires + other.fires,
        }
    }

    /// What was counted since `earlier` (a count taken before this one).
    pub fn since(self, earlier: Self) -> Self {
        Self {
            probes: self.probes - earlier.probes,
            writes: self.writes - earlier.writes,
            scans: self.scans - earlier.scans,
            fires: self.fires - earlier.fires,
        }
    }

    /// Whether nothing was counted.
    pub fn is_zero(self) -> bool {
        self == Self::default()
    }

    /// The modelled cost, nanoseconds, at `prices`.
    pub fn modelled_ns(self, prices: &Prices) -> u64 {
        self.probes * prices.probe_ns
            + self.writes * prices.write_ns
            + self.scans * prices.scan_ns
            + self.fires * prices.fire_ns
    }
}

/// Nanoseconds per counted operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Prices {
    /// One point read.
    pub probe_ns: u64,
    /// One row put or removed.
    pub write_ns: u64,
    /// One row visited by a scan.
    pub scan_ns: u64,
    /// One rule or operator invocation.
    pub fire_ns: u64,
}

impl Prices {
    /// The declared prices, calibrated by `bench.rs` (the report of C1 gives the measured values
    /// beside them).
    ///
    /// The table primitives measure, in the benchmark's isolating workloads, about 10 ns per
    /// probe, 26 per write (an insertion or removal; 10 to 15 for a replacement), 2.3 per scanned
    /// row, and nothing the benchmark can resolve per fire (a delta handled and dropped is inlined
    /// away; kept at 5 ns, as M1 kept its field read, because the benchmark cannot resolve it). The
    /// program around them (the rules, the clones and allocations, the rows' arithmetic) costs
    /// about twice that: the five `program/*` workloads, which run the rules over generated
    /// scenarios, measure 11 to 13 ns per counted operation (21 for the busiest), and these prices
    /// put each of them within 0.7 to 1.4 of its model (M1b's band). The primitives alone sit
    /// outside the band, at 0.1 to 0.4 of this model, because they are the part of the cost that
    /// the counts see directly; the rest is priced into the same four numbers.
    pub const DECLARED: Prices = Prices {
        probe_ns: 20,
        write_ns: 45,
        scan_ns: 5,
        fire_ns: 5,
    };
}

/// A change to one row of a logged table: `old` is `None` for an insertion, `new` is `None` for a
/// removal.
#[derive(Debug, Clone, PartialEq)]
pub struct Delta<K, V> {
    /// The row's key.
    pub key: K,
    /// The row before the change.
    pub old: Option<V>,
    /// The row after the change.
    pub new: Option<V>,
}

/// An ordered relation with counted operations and an optional delta log.
#[derive(Debug, Clone)]
pub struct Table<K, V> {
    rows: BTreeMap<K, V>,
    log: Option<Vec<Delta<K, V>>>,
    probes: Cell<u64>,
    writes: Cell<u64>,
    scans: Cell<u64>,
}

impl<K: Ord + Clone, V: Clone> Default for Table<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: Ord + Clone, V: Clone> Table<K, V> {
    /// An empty table that logs no deltas.
    pub fn new() -> Self {
        Self {
            rows: BTreeMap::new(),
            log: None,
            probes: Cell::new(0),
            writes: Cell::new(0),
            scans: Cell::new(0),
        }
    }

    /// An empty table that logs a delta for every change.
    pub fn logged() -> Self {
        Self {
            log: Some(Vec::new()),
            ..Self::new()
        }
    }

    /// The row at `key`. One probe.
    pub fn get(&self, key: &K) -> Option<&V> {
        self.probes.set(self.probes.get() + 1);
        self.rows.get(key)
    }

    /// Whether a row is at `key`. One probe.
    pub fn contains(&self, key: &K) -> bool {
        self.get(key).is_some()
    }

    /// The row at `key`, not counted: for the seam adapter.
    pub fn peek(&self, key: &K) -> Option<&V> {
        self.rows.get(key)
    }

    /// Put a row, replacing any at `key`. One write.
    pub fn put(&mut self, key: K, value: V) {
        self.writes.set(self.writes.get() + 1);
        match self.log.as_mut() {
            Some(log) => {
                let old = self.rows.insert(key.clone(), value.clone());
                log.push(Delta {
                    key,
                    old,
                    new: Some(value),
                });
            }
            None => {
                self.rows.insert(key, value);
            }
        }
    }

    /// Remove the row at `key`, returning it. One write if there was a row, none if not.
    pub fn del(&mut self, key: &K) -> Option<V> {
        let old = self.rows.remove(key);
        if old.is_some() {
            self.writes.set(self.writes.get() + 1);
            if let Some(log) = self.log.as_mut() {
                log.push(Delta {
                    key: key.clone(),
                    old: old.clone(),
                    new: None,
                });
            }
        }
        old
    }

    /// The rows whose keys are in `range`, in key order, each one counted when it is yielded.
    pub fn range<R: RangeBounds<K>>(&self, range: R) -> Scan<'_, K, V> {
        Scan {
            inner: self.rows.range(range),
            scans: &self.scans,
        }
    }

    /// The rows in `range`, in key order, not counted: for the seam adapter.
    pub fn scan_free<R: RangeBounds<K>>(&self, range: R) -> Range<'_, K, V> {
        self.rows.range(range)
    }

    /// The smallest key, if any. One probe.
    pub fn first_key(&self) -> Option<K> {
        self.probes.set(self.probes.get() + 1);
        self.rows.keys().next().cloned()
    }

    /// How many rows there are.
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Whether there are none.
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// The deltas logged since the last drain, in order. Empty for an unlogged table.
    pub fn drain(&mut self) -> Vec<Delta<K, V>> {
        self.log.as_mut().map(std::mem::take).unwrap_or_default()
    }

    /// What this table has counted so far (never any fires).
    pub fn counts(&self) -> OpCounts {
        OpCounts {
            probes: self.probes.get(),
            writes: self.writes.get(),
            scans: self.scans.get(),
            fires: 0,
        }
    }
}

/// A counted range scan: yields rows in key order, either direction.
pub struct Scan<'a, K, V> {
    inner: Range<'a, K, V>,
    scans: &'a Cell<u64>,
}

impl<'a, K, V> Iterator for Scan<'a, K, V> {
    type Item = (&'a K, &'a V);

    fn next(&mut self) -> Option<Self::Item> {
        let item = self.inner.next();
        if item.is_some() {
            self.scans.set(self.scans.get() + 1);
        }
        item
    }
}

impl<K, V> DoubleEndedIterator for Scan<'_, K, V> {
    fn next_back(&mut self) -> Option<Self::Item> {
        let item = self.inner.next_back();
        if item.is_some() {
            self.scans.set(self.scans.get() + 1);
        }
        item
    }
}

/// Keep `out` equal to the projection `f` of a table, given that table's `deltas`: for each delta
/// the row's image before and after is computed, and `out` is touched only if the two differ (so a
/// change that does not move the projection costs no write). `f` returning `None` filters the row
/// out. Returns the number of deltas handled, which the caller counts as fires.
pub fn project<K, V, K2, V2>(
    deltas: &[Delta<K, V>],
    out: &mut Table<K2, V2>,
    f: impl Fn(&K, &V) -> Option<(K2, V2)>,
) -> u64
where
    K2: Ord + Clone,
    V2: Clone + PartialEq,
{
    for d in deltas {
        let before = d.old.as_ref().and_then(|v| f(&d.key, v));
        let after = d.new.as_ref().and_then(|v| f(&d.key, v));
        if before == after {
            continue;
        }
        if let Some((k, _)) = before {
            out.del(&k);
        }
        if let Some((k, v)) = after {
            out.put(k, v);
        }
    }
    deltas.len() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_probes_writes_and_scans() {
        let mut t: Table<u32, u32> = Table::new();
        t.put(1, 10);
        t.put(2, 20);
        t.put(3, 30);
        assert_eq!(t.get(&2), Some(&20));
        assert_eq!(t.del(&9), None);
        assert_eq!(t.del(&1), Some(10));
        let rows: Vec<_> = t.range(2..).map(|(k, v)| (*k, *v)).collect();
        assert_eq!(rows, vec![(2, 20), (3, 30)]);
        let back: Vec<_> = t.range(..).rev().map(|(k, _)| *k).collect();
        assert_eq!(back, vec![3, 2]);
        let c = t.counts();
        assert_eq!((c.probes, c.writes, c.scans, c.fires), (1, 4, 4, 0));
        assert_eq!(t.peek(&3), Some(&30));
        assert_eq!(t.scan_free(..).count(), 2);
        assert_eq!(t.counts(), c, "peek and scan_free do not count");
    }

    #[test]
    fn a_logged_table_records_every_change_and_drains_once() {
        let mut t: Table<u32, u32> = Table::logged();
        t.put(1, 10);
        t.put(1, 11);
        t.del(&1);
        t.del(&1);
        let d = t.drain();
        assert_eq!(
            d,
            vec![
                Delta {
                    key: 1,
                    old: None,
                    new: Some(10)
                },
                Delta {
                    key: 1,
                    old: Some(10),
                    new: Some(11)
                },
                Delta {
                    key: 1,
                    old: Some(11),
                    new: None
                },
            ]
        );
        assert!(t.drain().is_empty());
        let mut plain: Table<u32, u32> = Table::new();
        plain.put(1, 1);
        assert!(plain.drain().is_empty());
    }

    #[test]
    fn project_touches_the_output_only_when_the_image_changes() {
        let mut src: Table<u32, (u32, bool)> = Table::logged();
        let mut idx: Table<(u32, u32), ()> = Table::new();
        let f = |k: &u32, v: &(u32, bool)| v.1.then_some(((v.0, *k), ()));
        src.put(1, (7, true));
        src.put(2, (7, false));
        assert_eq!(project(&src.drain(), &mut idx, f), 2);
        assert_eq!(idx.len(), 1);
        let w = idx.counts().writes;
        src.put(1, (7, true));
        project(&src.drain(), &mut idx, f);
        assert_eq!(idx.counts().writes, w, "an unchanged image is not written");
        src.put(1, (8, true));
        project(&src.drain(), &mut idx, f);
        assert!(idx.peek(&(8, 1)).is_some() && idx.peek(&(7, 1)).is_none());
        src.del(&1);
        project(&src.drain(), &mut idx, f);
        assert!(idx.is_empty());
    }

    #[test]
    fn counts_add_subtract_and_price() {
        let a = OpCounts {
            probes: 4,
            writes: 3,
            scans: 2,
            fires: 1,
        };
        let b = OpCounts {
            probes: 1,
            writes: 1,
            scans: 1,
            fires: 1,
        };
        assert_eq!(a.plus(b).since(b), a);
        assert!(OpCounts::default().is_zero() && !a.is_zero());
        let p = Prices {
            probe_ns: 1,
            write_ns: 10,
            scan_ns: 100,
            fire_ns: 1000,
        };
        assert_eq!(a.modelled_ns(&p), 4 + 30 + 200 + 1000);
    }
}
