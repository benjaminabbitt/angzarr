//! Gap analysis for handler-relative EventBook completeness.
//!
//! Determines whether an EventBook is complete relative to a handler's
//! checkpoint. This is the core decision logic with no I/O - pure functions only.

/// Result of analyzing an EventBook for gaps relative to a handler's checkpoint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GapAnalysis {
    /// EventBook is complete: first event sequence <= checkpoint + 1.
    /// Handler can process without fetching additional events.
    Complete,

    /// Gap exists: events between checkpoint and first_event are missing.
    /// Need to fetch events from (checkpoint + 1) to (first_event - 1).
    Gap {
        /// Handler's last processed sequence.
        checkpoint: u32,
        /// First event sequence in the received EventBook.
        first_event_seq: u32,
    },

    /// Handler has never seen this aggregate (no checkpoint exists).
    /// Need to fetch full history from sequence 0.
    NewAggregate,
}

/// Analyze whether an EventBook has a gap relative to a handler's checkpoint.
///
/// # Arguments
/// * `checkpoint` - Handler's last processed sequence for this (domain, root), or None if new
/// * `first_event_seq` - Sequence number of the first event in the received EventBook
///
/// # Returns
/// * `Complete` - No gap, EventBook can be processed as-is
/// * `Gap` - Missing events between checkpoint and first_event
/// * `NewAggregate` - No checkpoint exists, need full history from 0
pub fn analyze_gap(checkpoint: Option<u32>, first_event_seq: u32) -> GapAnalysis {
    match checkpoint {
        None => GapAnalysis::NewAggregate,
        Some(cp) if first_event_seq <= cp.saturating_add(1) => GapAnalysis::Complete,
        Some(cp) => GapAnalysis::Gap {
            checkpoint: cp,
            first_event_seq,
        },
    }
}

#[cfg(test)]
#[path = "analysis.test.rs"]
mod tests;
