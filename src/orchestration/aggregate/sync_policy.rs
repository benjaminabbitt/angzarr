//! Sync-mode policy for aggregate post-persist behavior.
//!
//! Centralizes the rule that gates whether post-persist waits for projectors.
//! Local and gRPC aggregate contexts both call this — keeping the rule in one
//! place means they cannot drift when sync modes are added or repurposed.

/// Returns true when the aggregate must wait for sync projectors before
/// returning to the caller.
///
/// SIMPLE and CASCADE wait. ASYNC and DECISION do not: ASYNC is fire-and-forget,
/// DECISION returns after the aggregate's accept/reject so the caller (typically
/// a process manager) can react to the decision without paying for projector
/// propagation. `None` (no sync mode set) defaults to skip.
pub fn should_call_sync_projectors(sync_mode: Option<crate::proto::SyncMode>) -> bool {
    matches!(
        sync_mode,
        Some(crate::proto::SyncMode::Simple) | Some(crate::proto::SyncMode::Cascade)
    )
}

#[cfg(test)]
#[path = "sync_policy.test.rs"]
mod tests;
