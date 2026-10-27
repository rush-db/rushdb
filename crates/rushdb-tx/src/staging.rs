//! Retained staging representation. Incoming decoded request objects and
//! commit-time decoding/planning are separate, still-unbudgeted allocations.
use rushdb_lmpg::{BatchOp, FieldChange};
use rushdb_storage::resources::{ByteBudget, ByteReservation};
use std::io::{self, Write};

const OP_METADATA_ALLOWANCE: usize = 256;
const MAX_JSON_DEPTH: usize = 64;

pub(crate) struct StagedOp {
    bytes: Vec<u8>,
    // Drop bytes before releasing their reservation.
    _reservation: ByteReservation,
}

struct Counter {
    bytes: usize,
    limit: usize,
}
impl Write for Counter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let total = self
            .bytes
            .checked_add(bytes.len())
            .filter(|total| *total <= self.limit)
            .ok_or_else(|| {
                io::Error::other("resource exhausted: transaction staging byte limit")
            })?;
        self.bytes = total;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn validate_value(
    value: &serde_json::Value,
    depth: usize,
    remaining_nodes: &mut usize,
) -> Result<(), String> {
    if depth > MAX_JSON_DEPTH || *remaining_nodes == 0 {
        return Err("resource exhausted: transaction JSON depth/size limit".into());
    }
    *remaining_nodes -= 1;
    match value {
        serde_json::Value::Array(values) => {
            for value in values {
                validate_value(value, depth + 1, remaining_nodes)?;
            }
        }
        serde_json::Value::Object(values) => {
            for value in values.values() {
                validate_value(value, depth + 1, remaining_nodes)?;
            }
        }
        _ => {}
    }
    Ok(())
}

impl StagedOp {
    pub(crate) fn encode(op: &BatchOp, limit: usize, budget: &ByteBudget) -> Result<Self, String> {
        let fields: &[(String, FieldChange)] = match op {
            BatchOp::CreateRecord { fields, .. }
            | BatchOp::UpdateFields { fields, .. }
            | BatchOp::CreateRelationship { fields, .. }
            | BatchOp::CreateRelationshipById { fields, .. } => fields,
            _ => &[],
        };
        let mut remaining_nodes = limit;
        for (_, change) in fields {
            if let FieldChange::Set(value) = change {
                validate_value(value, 0, &mut remaining_nodes)?;
            }
        }
        // Count without allocating an output buffer. The owned input cannot
        // change between these two deterministic serializations.
        let mut count = Counter { bytes: 0, limit };
        serde_json::to_writer(&mut count, op).map_err(|e| e.to_string())?;
        let charge = count
            .bytes
            .checked_add(OP_METADATA_ALLOWANCE)
            .ok_or("resource exhausted: staging size overflow")?;
        let mut reservation = budget
            .try_reserve(charge)
            .map_err(|e| format!("resource exhausted: {e}"))?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(count.bytes)
            .map_err(|e| format!("resource exhausted: {e}"))?;
        reservation
            .resize(
                bytes
                    .capacity()
                    .checked_add(OP_METADATA_ALLOWANCE)
                    .ok_or("resource exhausted: staging capacity overflow")?,
            )
            .map_err(|e| format!("resource exhausted: {e}"))?;
        serde_json::to_writer(&mut bytes, op).map_err(|e| e.to_string())?;
        debug_assert_eq!(bytes.len(), count.bytes);
        Ok(Self {
            bytes,
            _reservation: reservation,
        })
    }

    pub(crate) fn len(&self) -> usize {
        self.bytes.len()
    }
}
