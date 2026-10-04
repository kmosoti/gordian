//! Serde adapters for `gordian_core::Instant`, which core does not make serializable.
//!
//! Instants are written as plain `u64` nanoseconds. A timed stream is written as a sequence of
//! `[nanoseconds, observation]` pairs.

pub(crate) mod instant {
    use gordian_core::Instant;
    use serde::{Deserialize, Deserializer, Serializer};

    pub(crate) fn serialize<S: Serializer>(value: &Instant, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_u64(value.0)
    }

    pub(crate) fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Instant, D::Error> {
        u64::deserialize(d).map(Instant)
    }
}

pub(crate) mod timed {
    use crate::sense::Observation;
    use gordian_core::Instant;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub(crate) fn serialize<S: Serializer>(
        value: &[(Instant, Observation)],
        s: S,
    ) -> Result<S::Ok, S::Error> {
        let pairs: Vec<(u64, &Observation)> = value.iter().map(|(t, o)| (t.0, o)).collect();
        pairs.serialize(s)
    }

    pub(crate) fn deserialize<'de, D: Deserializer<'de>>(
        d: D,
    ) -> Result<Vec<(Instant, Observation)>, D::Error> {
        let pairs = Vec::<(u64, Observation)>::deserialize(d)?;
        Ok(pairs.into_iter().map(|(t, o)| (Instant(t), o)).collect())
    }
}
