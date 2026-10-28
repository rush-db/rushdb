//! Explicit per-request tenant scope.
//!
//! Replaces the former ambient thread-local: the scope is now a value
//! constructed once at the transport boundary, from the `scope: Option<String>`
//! the transport already authenticated (HTTP header, gRPC/Bolt metadata,
//! pgwire startup parameter — all resolve this explicitly before this type
//! ever exists), and threaded through `ProtocolExecutor` into every `Node`
//! method that needs it. No code path can read a tenant identity that
//! "happens to still be set" from an earlier, unrelated call — the value is
//! either in hand as a parameter or it doesn't exist.

use rushdb_types::ids::TenantId;

use super::Node;

/// Request-scoped view over a booted [`Node`]. `self.tenant()` resolves from
/// the value carried here, never from ambient state. `Deref`s to `Node` so
/// every method that doesn't need the tenant stays callable unchanged;
/// methods that do need it are defined on `RequestScope` instead of `Node`
/// directly, so they cannot compile without one.
///
/// `pub`, not `pub(crate)`: this is the replacement for the deleted
/// `Node::set_request_scope` as a calling convention, including for external
/// callers (tests, embedders) that need to act as a specific tenant — "set
/// the ambient scope, then call the method" becomes "construct a
/// `RequestScope` for that tenant, then call the method on it." Fields stay
/// private; a caller cannot build one except through [`Self::new`]/
/// [`Self::for_tenant`], so the tenant is always resolved the one documented
/// way.
#[derive(Clone)]
pub struct RequestScope<'a> {
    node: &'a Node,
    tenant: TenantId,
    /// Original scope string, when one was supplied (multitenant mode).
    /// Kept alongside the derived `tenant` because a few call sites (backup
    /// scope validation, the envelope-vs-transport scope cross-check)
    /// compare the raw string the caller supplied, not the derived id.
    scope: Option<String>,
    /// Why the scope could not be resolved through the tenant directory, if
    /// it could not (the tenant is then [`TenantId::UNREGISTERED`], which owns
    /// no keys). Surfaced by the executor before any operation runs.
    resolution_error: Option<String>,
    /// The request's own bounds, applied to every engine handle
    /// this scope builds.
    limits: rushdb_lmpg::RequestLimits,
}

impl<'a> RequestScope<'a> {
    /// Resolve a scope value (already authenticated by the transport) into a
    /// request scope through the tenant directory ([`Node::resolve_scope`]):
    /// no scope selects the node's base tenant, a new scope is registered on
    /// first use.
    #[must_use]
    pub fn new(node: &'a Node, scope: Option<String>) -> Self {
        let (tenant, resolution_error) = match node.resolve_scope(scope.as_deref()) {
            Ok(tenant) => {
                // A served tenant is resident: recency for the
                // per-tenant metric set and LRU eviction.
                node.residency.touch(tenant);
                (tenant, None)
            }
            Err(error) => (TenantId::UNREGISTERED, Some(error)),
        };
        RequestScope {
            node,
            tenant,
            scope,
            resolution_error,
            limits: rushdb_lmpg::RequestLimits::default(),
        }
    }

    /// Construct directly from an already-resolved tenant, for callers (the
    /// scheduler dispatch closures) that resolved the tenant before the
    /// scope string was available to move into the closure, or test code
    /// that wants a specific tenant without going through scope-string
    /// derivation.
    #[must_use]
    pub fn for_tenant(node: &'a Node, tenant: TenantId, scope: Option<String>) -> Self {
        RequestScope {
            node,
            tenant,
            scope,
            resolution_error: None,
            limits: rushdb_lmpg::RequestLimits::default(),
        }
    }

    /// This scope with a request's own bounds.
    #[must_use]
    pub fn with_request_limits(mut self, limits: rushdb_lmpg::RequestLimits) -> Self {
        self.limits = limits;
        self
    }

    /// The request's own bounds.
    #[must_use]
    pub fn request_limits(&self) -> rushdb_lmpg::RequestLimits {
        self.limits
    }

    /// Why this request's scope could not be resolved, if it could not.
    #[must_use]
    pub fn resolution_error(&self) -> Option<&str> {
        self.resolution_error.as_deref()
    }

    /// Tenant id for this request. Replaces the panicking ambient
    /// `scope::request_tenant()` — there is no "forgot to set the scope"
    /// state to panic on; a `RequestScope` cannot exist without one.
    #[must_use]
    pub fn tenant(&self) -> TenantId {
        self.tenant
    }

    /// Original scope string this request resolved from, when one was
    /// supplied. `None` in single-tenant mode.
    #[must_use]
    pub fn scope_value(&self) -> Option<&str> {
        self.scope.as_deref()
    }
}

impl std::ops::Deref for RequestScope<'_> {
    type Target = Node;

    fn deref(&self) -> &Node {
        self.node
    }
}
