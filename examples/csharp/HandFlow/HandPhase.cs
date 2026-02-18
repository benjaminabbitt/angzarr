namespace HandFlow;

/// <summary>
/// Internal state machine phases for hand orchestration.
/// </summary>
public enum HandPhase
{
    WaitingForStart,
    Dealing,
    PostingBlinds,
    Betting,
    DealingCommunity,
    Draw,
    Showdown,
    AwardingPot,
    Complete
}
