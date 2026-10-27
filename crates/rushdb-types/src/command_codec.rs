//! The one binary encoding of a [`CommitBatch`] (SD-7.3): WAL command
//! envelopes, retry-outcome payloads and replication records all carry these
//! bytes.
//!
//! The wire format is postcard's (specified, stable since 1.0): varint
//! integers, length-prefixed byte strings, enum variants by index, no field
//! names. Byte keys therefore cost their length plus one or two bytes, not a
//! JSON list of decimal numbers (the JSON command was ~3 KB per ingested
//! record and 80% of WAL bytes). Changing a command type changes the format;
//! [`COMMAND_VERSION`](crate::command::COMMAND_VERSION) is bumped with it.

use crate::command::CommitBatch;

/// Upper bound on a decoded command (the WAL's own record cap is lower);
/// rejects a corrupt length before it allocates.
const MAX_COMMAND_BYTES: usize = 1 << 30;

/// Malformed command bytes.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[error("malformed command: {0}")]
pub struct CommandCodecError(String);

/// Encode `batch`.
///
/// # Panics
/// Never in practice: every `CommitBatch` field is serializable and the
/// output is an in-memory vector.
#[must_use]
pub fn encode(batch: &CommitBatch) -> Vec<u8> {
    postcard::to_stdvec(batch).expect("a CommitBatch always encodes")
}

/// Decode command bytes produced by [`encode`]. Trailing bytes are an error:
/// a command is always exactly one batch.
///
/// # Errors
/// Returns [`CommandCodecError`] for truncated, oversized, trailing or
/// otherwise malformed input.
pub fn decode(bytes: &[u8]) -> Result<CommitBatch, CommandCodecError> {
    if bytes.len() > MAX_COMMAND_BYTES {
        return Err(CommandCodecError(format!("{} bytes", bytes.len())));
    }
    let (batch, rest) = postcard::take_from_bytes::<CommitBatch>(bytes)
        .map_err(|error| CommandCodecError(error.to_string()))?;
    if !rest.is_empty() {
        return Err(CommandCodecError(format!("{} trailing bytes", rest.len())));
    }
    if batch.command_version != crate::command::COMMAND_VERSION {
        return Err(CommandCodecError(format!(
            "command version {} (expected {})",
            batch.command_version,
            crate::command::COMMAND_VERSION
        )));
    }
    Ok(batch)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::{IsolationMode, Mutation, Precondition};
    use crate::ids::{OwnerKind, TenantId};
    use crate::model::LogicalType;
    use crate::occurrence::{OccurrenceRow, OccurrenceValue};

    /// Seeded xorshift: a reproducible property test without a dependency.
    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }
        fn bytes(&mut self, max: u64) -> Vec<u8> {
            (0..self.next() % max)
                .map(|_| self.next().to_le_bytes()[0])
                .collect()
        }
    }

    fn batch(rng: &mut Rng) -> CommitBatch {
        let mut mutations = Vec::new();
        for _ in 0..rng.next() % 8 {
            mutations.push(match rng.next() % 5 {
                0 => Mutation::CreateRecordHeader {
                    record_id: rng.next().to_le_bytes().repeat(2).try_into().unwrap(),
                    ordinal: rng.next(),
                    label: format!("L{}", rng.next() % 100),
                },
                1 => Mutation::PutOccurrence {
                    row: OccurrenceRow {
                        owner_kind: OwnerKind::Record,
                        owner_ord: rng.next() >> (rng.next() % 64),
                        property_id: rng.next() % 1000,
                        family_id: rng.next() % 1000,
                        elem_ordinal: rng.next().is_multiple_of(2).then(|| rng.next() % 10),
                        occurrence_value: OccurrenceValue::Stored {
                            ty: LogicalType::Utf8,
                        },
                    },
                    canonical_value_bytes: Some(rng.bytes(200)),
                },
                2 => Mutation::PutRawKey {
                    key: rng.bytes(64),
                    value: rng.bytes(1024),
                },
                3 => Mutation::SetMetaValue {
                    key: format!("k{}", rng.next()),
                    value: rng.next(),
                },
                _ => Mutation::IndexApplied {
                    index: u32::try_from(rng.next() % 50).unwrap(),
                    upto: rng.next(),
                    owners: (0..rng.next() % 20).map(|_| rng.next()).collect(),
                },
            });
        }
        CommitBatch {
            command_version: crate::command::COMMAND_VERSION,
            tenant: TenantId::from_ordinal(rng.next() % 100 + 1),
            transaction_id: format!("tx{}", rng.next() % 7),
            commit_key: uuid::Uuid::from_u128(
                u128::from(rng.next()) << 64 | u128::from(rng.next()),
            ),
            start_index: rng.next(),
            isolation: [
                IsolationMode::ReadCommitted,
                IsolationMode::Snapshot,
                IsolationMode::Serializable,
            ][usize::try_from(rng.next() % 3).unwrap()],
            preconditions: (0..rng.next() % 4)
                .map(|i| {
                    if i % 2 == 0 {
                        Precondition::absent(rng.bytes(40))
                    } else {
                        Precondition::exists(rng.bytes(40))
                    }
                })
                .collect(),
            mutations,
        }
    }

    #[test]
    fn command_codec_roundtrip_proptest_and_malformed_fuzz() {
        let mut rng = Rng(0x2545_f491_4f6c_dd1d);
        for _ in 0..2_000 {
            let original = batch(&mut rng);
            let bytes = encode(&original);
            assert_eq!(decode(&bytes).unwrap(), original);
            // Every strict prefix is truncated input: an error, never a panic.
            let cut = usize::try_from(rng.next()).unwrap() % bytes.len();
            assert!(decode(&bytes[..cut]).is_err());
            // Trailing garbage is rejected, not ignored.
            let mut longer = bytes.clone();
            longer.push(0);
            assert!(decode(&longer).is_err());
            // Random corruption decodes to an error or to some batch, never
            // a panic or an unbounded allocation.
            let mut corrupt = bytes;
            let at = usize::try_from(rng.next()).unwrap() % corrupt.len();
            corrupt[at] = rng.next().to_le_bytes()[0];
            let _ = decode(&corrupt);
        }
        for _ in 0..2_000 {
            let noise = rng.bytes(256);
            let _ = decode(&noise);
        }
    }

    #[test]
    fn foreign_command_version_is_rejected() {
        let mut rng = Rng(7);
        let mut stale = batch(&mut rng);
        stale.command_version -= 1;
        assert!(decode(&encode(&stale)).is_err());
    }

    #[test]
    fn a_threshold_policy_round_trips_after_the_earlier_variants() {
        use crate::security::{AuthorizationMode, DataAccessPolicy};
        let batch = CommitBatch {
            command_version: crate::command::COMMAND_VERSION,
            tenant: TenantId::from_ordinal(1),
            transaction_id: String::new(),
            commit_key: uuid::Uuid::nil(),
            start_index: 0,
            isolation: IsolationMode::Snapshot,
            preconditions: Vec::new(),
            mutations: vec![
                Mutation::SetSecurityPolicy {
                    policy: DataAccessPolicy::default(),
                    mode: AuthorizationMode::PolicyEnforced,
                },
                Mutation::SetSecurityPolicyThreshold {
                    policy: DataAccessPolicy::default(),
                    mode: AuthorizationMode::PolicyEnforced,
                    min_count: 5,
                },
            ],
        };
        let bytes = encode(&batch);
        // The earlier variant keeps its index (0): logs written before the
        // threshold existed decode unchanged.
        assert_eq!(decode(&bytes).unwrap(), batch);
    }
}
