//! Serde adapters for `gordian_core::Instant`, which core does not make serializable.
//!
//! Instants are written as plain `u64` nanoseconds, the same form the stream uses.

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

pub(crate) mod option_instant {
    use gordian_core::Instant;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub(crate) fn serialize<S: Serializer>(
        value: &Option<Instant>,
        s: S,
    ) -> Result<S::Ok, S::Error> {
        value.map(|i| i.0).serialize(s)
    }

    pub(crate) fn deserialize<'de, D: Deserializer<'de>>(
        d: D,
    ) -> Result<Option<Instant>, D::Error> {
        Option::<u64>::deserialize(d).map(|o| o.map(Instant))
    }
}
