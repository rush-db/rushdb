//! The type a record holds for a field, for `$type` predicates: the
//! declared type of the name, or, for a name stored under more than one
//! type, the type of the definition the record holds.

use rushdb_types::model::LogicalType;
use serde_json::{Map, Value as Json};

use crate::{access, normalize, root_ord_of, EvalCtx, QueryError, ID_FIELD};

fn type_of_field(
    ctx: &EvalCtx<'_>,
    name: &str,
) -> Result<Option<rushdb_types::model::LogicalType>, QueryError> {
    let norm = normalize(name);
    if let Some(known) = ctx.declared_types.borrow().get(&norm) {
        return Ok(*known);
    }
    let declared = declared_type_of(ctx, &norm)?;
    ctx.declared_types.borrow_mut().insert(norm, declared);
    Ok(declared)
}

/// The type of `name` on the record `obj`: the declared type, or for a name
/// stored under more than one type, the type of the definition the record
/// holds (`None` when it holds none).
pub(crate) fn type_held(
    ctx: &EvalCtx<'_>,
    name: &str,
    obj: &Map<String, Json>,
) -> Result<Option<LogicalType>, QueryError> {
    let norm = normalize(name);
    if !ctx.record_types.borrow().contains_key(&norm) {
        let split = split_definitions_of(ctx, &norm)?;
        ctx.record_types.borrow_mut().insert(norm.clone(), split);
    }
    let types = ctx.record_types.borrow();
    let split = &types[&norm];
    if split.is_empty() {
        drop(types);
        return type_of_field(ctx, name);
    }
    let Some(id) = obj.get(ID_FIELD).and_then(Json::as_str) else {
        return Ok(None);
    };
    let ordinal = root_ord_of(ctx, id)?;
    // Record reconstruction shows the highest pid a record holds.
    let mut held = None;
    for &(pid, ty) in split {
        let rows = access::occurrences_of_pid(
            &ctx.snap,
            ctx.lmpg.tenant,
            rushdb_types::ids::OwnerKind::Record,
            ordinal,
            pid,
        )?;
        if !rows.is_empty() {
            held = Some(ty);
        }
    }
    Ok(held)
}

/// The typed definitions of `norm` in pid order when they span more than one
/// type (integers and floats are one: numbers); empty otherwise.
fn split_definitions_of(
    ctx: &EvalCtx<'_>,
    norm: &str,
) -> Result<Vec<(u64, LogicalType)>, QueryError> {
    let sig = rushdb_lmpg::reader::keys_ext::family_sig_key(
        ctx.lmpg.tenant,
        "default",
        rushdb_types::ids::OwnerKind::Record,
        norm,
    );
    let Some(raw) = ctx.snap.get(&sig)? else {
        return Ok(Vec::new());
    };
    let fid = u64::from_be_bytes(
        raw.as_slice()
            .try_into()
            .map_err(|_| rushdb_storage::StorageError::CorruptLog)?,
    );
    let mut typed = Vec::new();
    for def in access::definitions_of_family(&ctx.snap, ctx.lmpg.tenant, fid)? {
        if let rushdb_types::model::TypeState::Known(ty) = def.type_state {
            typed.push((def.property_id, ty));
        }
    }
    typed.sort_unstable_by_key(|(pid, _)| *pid);
    let class = |ty: &LogicalType| match ty {
        LogicalType::Int64 | LogicalType::Float64 => LogicalType::Float64,
        other => *other,
    };
    let one_type = typed
        .windows(2)
        .all(|pair| class(&pair[0].1) == class(&pair[1].1));
    Ok(if one_type { Vec::new() } else { typed })
}

fn declared_type_of(
    ctx: &EvalCtx<'_>,
    norm: &str,
) -> Result<Option<rushdb_types::model::LogicalType>, QueryError> {
    let sig = rushdb_lmpg::reader::keys_ext::family_sig_key(
        ctx.lmpg.tenant,
        "default",
        rushdb_types::ids::OwnerKind::Record,
        norm,
    );
    let Some(raw) = ctx.snap.get(&sig)? else {
        return Ok(None);
    };
    let fid = u64::from_be_bytes(
        raw.as_slice()
            .try_into()
            .map_err(|_| rushdb_storage::StorageError::CorruptLog)?,
    );
    for def in access::definitions_of_family(&ctx.snap, ctx.lmpg.tenant, fid)? {
        if let rushdb_types::model::TypeState::Known(ty) = def.type_state {
            return Ok(Some(ty));
        }
    }
    Ok(None)
}
