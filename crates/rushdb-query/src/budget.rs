//! The two byte budgets a read is charged against, and the error that names
//! the one that tripped.
//!
//! A read holds a *working set* while it runs (candidate and match ordinals,
//! ordering rows, the row being evaluated, aggregate buckets): that is
//! `query-memory-max`, tightened by the request. What it *returns* is charged
//! by its real serialized bytes against the *response* budget: the response
//! size limit (`server.body-limit`), or `query-memory-max` when the caller
//! gave no response budget.

use crate::QueryError;

/// Which byte budget a read exceeded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Budget {
    /// `query-memory-max`: what a query holds while it runs.
    WorkingSet,
    /// The response size limit (`server.body-limit`): the serialized rows a
    /// query returns.
    Response,
}

impl Budget {
    /// The error for a read that would hold or return more than `limit`
    /// bytes: it names the limit and the flag that raises it.
    #[must_use]
    pub fn exceeded(self, limit: u64) -> QueryError {
        QueryError::ResourceLimit(match self {
            Self::WorkingSet => format!(
                "query working set would exceed query-memory-max ({limit} bytes); \
                 raise --query-memory-max or narrow the filter"
            ),
            Self::Response => format!(
                "response would exceed the response size limit ({limit} bytes); \
                 raise --body-limit or request a smaller page"
            ),
        })
    }

    /// Fails when `bytes` exceed `limit` (`None`: uncapped).
    pub(crate) fn check(self, limit: Option<usize>, bytes: usize) -> Result<(), QueryError> {
        match limit {
            Some(limit) if bytes > limit => Err(self.exceeded(as_u64(limit))),
            _ => Ok(()),
        }
    }
}

/// One budget with what is already charged to it, handed down to work that
/// spends from the same allowance (a page's `$collect` expansion).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Cap {
    kind: Budget,
    limit: usize,
    used: usize,
}

impl Cap {
    pub(crate) fn new(kind: Budget, limit: usize) -> Self {
        Self {
            kind,
            limit,
            used: 0,
        }
    }

    /// The allowance a read gets: the caller's response budget, else
    /// `query-memory-max` (`0` = off).
    ///
    /// `Some(0)` from a caller is a *hard zero cap* (an exhausted request
    /// budget), distinct from a configured `query-memory-max` of `0`, which
    /// disables the cap.
    pub(crate) fn for_response(response: Option<usize>, configured: Option<u64>) -> Option<Self> {
        match response {
            Some(bytes) => Some(Self::new(Budget::Response, bytes)),
            None => configured.filter(|bytes| *bytes != 0).map(|bytes| {
                Self::new(
                    Budget::WorkingSet,
                    usize::try_from(bytes).unwrap_or(usize::MAX),
                )
            }),
        }
    }

    /// Fails when `bytes` more than what is already charged exceed the limit.
    pub(crate) fn check(self, bytes: usize) -> Result<(), QueryError> {
        if self.used.saturating_add(bytes) > self.limit {
            return Err(self.kind.exceeded(as_u64(self.limit)));
        }
        Ok(())
    }

    /// [`Self::check`] when there is a cap.
    pub(crate) fn check_opt(cap: Option<Self>, bytes: usize) -> Result<(), QueryError> {
        cap.map_or(Ok(()), |cap| cap.check(bytes))
    }

    /// The same budget with `bytes` more already charged.
    pub(crate) fn after(self, bytes: usize) -> Self {
        Self {
            used: self.used.saturating_add(bytes),
            ..self
        }
    }

    /// Bytes still free.
    pub(crate) fn remaining(self) -> usize {
        self.limit.saturating_sub(self.used)
    }

    pub(crate) fn limit(self) -> usize {
        self.limit
    }
}

/// The bytes a search holds at once, against `query-memory-max`.
pub(crate) struct WorkingSet {
    pub(crate) limit: Option<u64>,
    pub(crate) charged: u64,
}

impl WorkingSet {
    /// Admit `bytes`; `retained` bytes stay charged, others (a record that
    /// only feeds the predicate) only need to fit in the remaining room.
    pub(crate) fn charge(&mut self, bytes: u64, retained: bool) -> Result<(), QueryError> {
        if let Some(limit) = self.limit {
            if self.charged.saturating_add(bytes) > limit {
                return Err(Budget::WorkingSet.exceeded(limit));
            }
            if retained {
                self.charged += bytes;
            }
        }
        Ok(())
    }
}

fn as_u64(bytes: usize) -> u64 {
    u64::try_from(bytes).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::{Budget, Cap, WorkingSet};
    use crate::QueryError;

    fn message(error: QueryError) -> String {
        match error {
            QueryError::ResourceLimit(message) => message,
            other => panic!("expected a resource limit, got {other:?}"),
        }
    }

    #[test]
    fn each_budget_names_its_own_limit_and_flag() {
        let working = message(Budget::WorkingSet.check(Some(10), 11).unwrap_err());
        assert!(working.contains("query-memory-max (10 bytes)"), "{working}");
        assert!(working.contains("--query-memory-max"), "{working}");
        assert!(!working.contains("body-limit"), "{working}");
        let response = message(Cap::new(Budget::Response, 10).check(11).unwrap_err());
        assert!(
            response.contains("response size limit (10 bytes)"),
            "{response}"
        );
        assert!(response.contains("--body-limit"), "{response}");
        assert!(!response.contains("query-memory-max"), "{response}");
        let mut set = WorkingSet {
            limit: Some(8),
            charged: 0,
        };
        set.charge(8, true).unwrap();
        let records = message(set.charge(1, false).unwrap_err());
        assert_eq!(records, working.replace("(10 bytes)", "(8 bytes)"));
    }

    #[test]
    fn a_cap_spends_from_one_allowance_and_reports_the_whole_limit() {
        let cap = Cap::new(Budget::Response, 100).after(60);
        assert_eq!(cap.remaining(), 40);
        cap.check(40).unwrap();
        let error = message(cap.check(41).unwrap_err());
        assert!(error.contains("(100 bytes)"), "{error}");
        assert!(Cap::for_response(None, Some(0)).is_none());
        assert_eq!(Cap::for_response(Some(0), None).unwrap().remaining(), 0);
        let fallback = Cap::for_response(None, Some(5)).unwrap();
        assert!(message(fallback.check(6).unwrap_err()).contains("query-memory-max"));
    }
}
