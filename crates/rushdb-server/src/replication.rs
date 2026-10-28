//! Streaming replication: a follower node tails a leader's
//! WAL and replays each committed batch locally through
//! [`rushdb_storage::Store::apply_replicated_batch_with_payload`]. Derived
//! state is never shipped — only recomputed, deterministically, from the
//! same mutations the leader applied.
//!
//! A follower starts from an empty store (tailing from LSN 1) or from a
//! base backup of the leader (mid-stream attach, tailing from the backup's
//! commit index). A follower's local commit index is assigned sequentially
//! by `apply_replicated_batch`, so it stays aligned with the leader's LSN
//! numbering only when every batch after its starting point is replayed in
//! order, with no gaps (see [`rushdb_storage::Store::try_log_commands_from`]'s
//! doc comment for why normal operation never produces a gap).
//!
//! Tailing from LSN 1 needs the leader to still hold that history. The
//! default retention keeps only what a registered consumer pins (DC4,
//! 2026-10-10), and a follower registers on its first poll, so a follower
//! that starts empty after the leader has pruned gets `resync_required` and
//! must attach from a base backup instead. Leaders that serve
//! bootstrap-from-empty followers set `tx-log-keep-commits`.
//!
//! Internal, HTTP-only mechanism — not part of the client-facing protocol
//! surface, the same way `Node::export_snapshot_at`/`restore_pitr_to` are
//! internal.

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;

use crate::node::Node;
use crate::retention::ReplicaConfig;

/// Bound on records served per poll, keeping one response (and the backing
/// WAL read) a reasonable size even when a follower is far behind.
const WAL_TAIL_PAGE_LIMIT: usize = 2_000;

/// How long a caught-up (or errored) follower waits before polling again.
const FOLLOWER_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(200);

/// One WAL record ready to replay: an LSN and its base64-encoded,
/// JSON-serialized `CommitBatch`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WalRecord {
    /// The commit's log sequence number on the leader.
    pub lsn: u64,
    /// Base64-encoded JSON `CommitBatch`.
    pub command_b64: String,
}

/// Leader response to a follower's WAL-tail poll.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WalTailResponse {
    /// Records with LSN `>= since`, in order, bounded to
    /// [`WAL_TAIL_PAGE_LIMIT`].
    pub records: Vec<WalRecord>,
    /// `true` when `records` is everything the leader currently has for
    /// this follower (no page limit was hit).
    pub caught_up: bool,
}

/// Why a WAL-tail poll could not be served.
#[derive(Debug)]
pub(crate) enum WalTailError {
    /// The follower asked for history retention already reclaimed:
    /// it must be rebuilt from a fresh base backup.
    Resync(crate::retention::ConsumerResyncRequired),
    /// The WAL read failed.
    Read(String),
}

/// Build the leader-side response to a follower's poll: registers the
/// follower's retention floor at `since` *before* reading (so a concurrent
/// GC tick cannot truncate a segment this poll is about to serve), then
/// reads and bounds the result to [`WAL_TAIL_PAGE_LIMIT`].
///
/// # Errors
/// [`WalTailError::Resync`] when `since` is below the retention floor;
/// [`WalTailError::Read`] when the underlying WAL read fails.
pub(crate) fn build_wal_tail_response(
    node: &Node,
    follower_id: &str,
    since: u64,
) -> Result<WalTailResponse, WalTailError> {
    node.acknowledge_retention_consumer(format!("replica-{follower_id}"), since)
        .map_err(WalTailError::Resync)?;
    let all = node
        .store()
        .try_log_commands_from(since)
        .map_err(|error| WalTailError::Read(error.to_string()))?;
    let caught_up = all.len() <= WAL_TAIL_PAGE_LIMIT;
    let records = all
        .into_iter()
        .take(WAL_TAIL_PAGE_LIMIT)
        .map(|(lsn, command)| WalRecord {
            lsn,
            command_b64: STANDARD.encode(command),
        })
        .collect();
    Ok(WalTailResponse { records, caught_up })
}

/// Follower loop: repeatedly polls the leader's WAL-tail endpoint from this
/// node's own `committed_version() + 1` and replays each returned batch
/// locally. Runs until the node is dropped; network and decode errors are
/// logged and retried, never fatal to the process. An `apply` failure
/// (e.g. the leader skipped ahead of what this follower can replay) is
/// also retried — it is not expected to self-heal, but crashing a
/// follower over it would be worse than a visibly stalled replication lag.
pub(crate) async fn run_replication_follower(node: std::sync::Weak<Node>, replica: ReplicaConfig) {
    let client = reqwest::Client::new();
    loop {
        let Some(strong) = node.upgrade() else {
            return;
        };
        if strong.follower_stop_requested() {
            // Only this task ever clears `following` — see
            // `Node::finish_following`'s doc comment for why that
            // indirection matters (no window where a write could race a
            // batch this loop is still mid-apply on).
            strong.finish_following();
            return;
        }
        let since = strong.committed_version() + 1;
        drop(strong);

        let Ok(mut url) = reqwest::Url::parse(replica.leader_url.trim_end_matches('/')) else {
            tracing::error!(
                leader_url = %replica.leader_url,
                "replication follower: invalid leader URL, stopping"
            );
            return;
        };
        url.set_path("/internal/replication/wal");
        url.query_pairs_mut()
            .append_pair("follower_id", &replica.follower_id)
            .append_pair("since", &since.to_string());

        let page = match fetch_wal_page(&client, url, &replica.token).await {
            Ok(page) => page,
            Err(FetchError::Resync(oldest)) => {
                // Not retryable: the leader no longer has the history this
                // follower needs. Rebuild it from a base backup.
                tracing::error!(
                    oldest_available_lsn = oldest,
                    since,
                    "replication follower: leader requires resync from a fresh base; stopping"
                );
                return;
            }
            Err(FetchError::Other(error)) => {
                tracing::warn!(%error, "replication follower: poll failed, retrying");
                tokio::time::sleep(FOLLOWER_POLL_INTERVAL).await;
                continue;
            }
        };

        let Some(strong) = node.upgrade() else {
            return;
        };
        let mut apply_failed = false;
        for record in &page.records {
            if let Err(error) = decode_and_apply(&strong, record) {
                tracing::error!(
                    %error,
                    lsn = record.lsn,
                    "replication follower: apply failed, retrying"
                );
                apply_failed = true;
                break;
            }
        }
        let empty = page.records.is_empty();
        drop(strong);

        if apply_failed || (empty && page.caught_up) {
            tokio::time::sleep(FOLLOWER_POLL_INTERVAL).await;
        }
    }
}

enum FetchError {
    /// `resync_required`: the oldest LSN the leader still has.
    Resync(u64),
    Other(String),
}

async fn fetch_wal_page(
    client: &reqwest::Client,
    url: reqwest::Url,
    token: &str,
) -> Result<WalTailResponse, FetchError> {
    let response = client
        .get(url)
        .bearer_auth(token)
        .send()
        .await
        .map_err(|error| FetchError::Other(error.to_string()))?;
    if response.status() == reqwest::StatusCode::CONFLICT {
        let body: serde_json::Value = response
            .json()
            .await
            .map_err(|error| FetchError::Other(error.to_string()))?;
        if let Some(oldest) = body
            .pointer("/error/oldestAvailableLsn")
            .and_then(serde_json::Value::as_u64)
        {
            return Err(FetchError::Resync(oldest));
        }
        return Err(FetchError::Other(format!("conflict: {body}")));
    }
    response
        .error_for_status()
        .map_err(|error| FetchError::Other(error.to_string()))?
        .json::<WalTailResponse>()
        .await
        .map_err(|error| FetchError::Other(error.to_string()))
}

fn decode_and_apply(node: &Node, record: &WalRecord) -> Result<(), String> {
    let raw = STANDARD
        .decode(&record.command_b64)
        .map_err(|error| format!("decode WAL record: {error}"))?;
    let batch = rushdb_types::command_codec::decode(&raw)
        .map_err(|error| format!("deserialize CommitBatch: {error}"))?;
    node.store()
        .apply_replicated_batch_with_payload(&batch, None)
        .map_err(|error| error.to_string())?;
    Ok(())
}
