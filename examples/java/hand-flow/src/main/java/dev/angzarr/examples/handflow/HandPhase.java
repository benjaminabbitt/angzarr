package dev.angzarr.examples.handflow;

/**
 * Internal state machine phases for hand orchestration.
 */
public enum HandPhase {
    WAITING_FOR_START,
    DEALING,
    POSTING_BLINDS,
    BETTING,
    DEALING_COMMUNITY,
    DRAW,
    SHOWDOWN,
    AWARDING_POT,
    COMPLETE
}
