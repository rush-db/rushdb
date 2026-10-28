//! Logical backup and restore for `RushDB`.
//!
//! This crate owns the JSONL backup surface only. It is **not** a migration
//! tool: migrations from other stores are performed outside the engine (see
//! the manual's "Upgrading from `RushDB` 2.x"). The surface is:
//!
//! - streaming tenant JSONL export ([`export`]); and
//! - JSONL backup restore ([`restore_backup`], [`restore_backup_reader`]).

pub mod export;

mod backup;
pub use backup::{export_tenant_jsonl, restore_backup, restore_backup_reader};
pub use export::export_tenant_jsonl_to;
