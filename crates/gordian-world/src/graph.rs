//! The service dependency graph. Everything in this module is public knowledge at episode start.

use serde::{Deserialize, Serialize};

/// Index of a service in its world. Dense and zero-based.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ServiceId(pub u32);

impl ServiceId {
    /// The index as a `usize`, for slicing.
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

/// The kind of resource a service is bounded by. It colours nothing in the rules; it is part of
/// the graph a component may reason over.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResourceKind {
    /// CPU-bound.
    Cpu,
    /// Memory-bound.
    Memory,
    /// Disk-bound.
    Disk,
    /// Network-bound.
    Network,
}

impl ResourceKind {
    /// Every resource kind, in declaration order.
    pub const ALL: [ResourceKind; 4] = [
        ResourceKind::Cpu,
        ResourceKind::Memory,
        ResourceKind::Disk,
        ResourceKind::Network,
    ];
}

/// One service.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Service {
    /// Position in the world.
    pub id: ServiceId,
    /// Services this one depends on. Every entry has a smaller index than `id` (the graph is a
    /// DAG by construction), sorted ascending, without duplicates.
    pub depends_on: Vec<ServiceId>,
    /// What the service is bounded by.
    pub resource: ResourceKind,
    /// Hash of the service's configuration at episode start.
    pub config_hash: u64,
    /// Public fact: this service's health endpoint cannot decide. A `HealthCheck` against it
    /// returns `Inconclusive` and suggests running the same check again (see `physics`).
    pub unreliable_health: bool,
}

/// The public part of a world: its services. Faults and hidden bits are not here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct World {
    /// Services, indexed by `ServiceId`.
    pub services: Vec<Service>,
}

impl World {
    /// Number of services.
    pub fn len(&self) -> usize {
        self.services.len()
    }

    /// True when the world has no services.
    pub fn is_empty(&self) -> bool {
        self.services.is_empty()
    }

    /// The service with this id, if it exists.
    pub fn service(&self, id: ServiceId) -> Option<&Service> {
        self.services.get(id.index())
    }

    /// Every dependency edge as `(dependent, dependency)`.
    pub fn edges(&self) -> Vec<(ServiceId, ServiceId)> {
        self.services
            .iter()
            .flat_map(|s| s.depends_on.iter().map(move |d| (s.id, *d)))
            .collect()
    }

    /// Services that depend on `site`, directly or transitively, in ascending id order.
    pub fn dependents_of(&self, site: ServiceId) -> Vec<ServiceId> {
        dependents_mask(&self.services, site)
            .iter()
            .enumerate()
            .filter(|(_, hit)| **hit)
            .map(|(i, _)| ServiceId(i as u32))
            .collect()
    }
}

/// `mask[i]` is true when service `i` depends on `site`, directly or transitively. `site` itself
/// is not marked. Relies on edges pointing from higher to lower index.
pub fn dependents_mask(services: &[Service], site: ServiceId) -> Vec<bool> {
    let mut reach = vec![false; services.len()];
    let start = site.index();
    if start >= services.len() {
        return reach;
    }
    let mut member = vec![false; services.len()];
    member[start] = true;
    for i in (start + 1)..services.len() {
        if services[i].depends_on.iter().any(|d| member[d.index()]) {
            member[i] = true;
            reach[i] = true;
        }
    }
    reach
}
