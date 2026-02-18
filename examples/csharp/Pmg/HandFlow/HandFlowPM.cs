using Angzarr.Client;
using Angzarr.Proto.Angzarr;
using Angzarr.Proto.Examples;

namespace Angzarr.Examples.PmgHandFlow;

/// <summary>
/// Hand Flow Process Manager - orchestrates poker hand phases across domains.
///
/// This PM coordinates the workflow between table and hand domains,
/// tracking phase transitions and dispatching commands as the hand progresses.
/// </summary>

// docs:start:pm_state
public enum HandPhase { AwaitingDeal, Dealing, Blinds, Betting, Complete }

public class HandFlowState
{
    public string HandId { get; set; } = "";
    public HandPhase Phase { get; set; } = HandPhase.AwaitingDeal;
    public int PlayerCount { get; set; } = 0;
}
// docs:end:pm_state

// docs:start:pm_handler
public class HandFlowPM : ProcessManager<HandFlowState>
{
    [ReactsTo(typeof(HandStarted))]
    public IEnumerable<CommandBook> HandleHandStarted(HandStarted evt, HandFlowState state)
    {
        state.HandId = evt.HandId;
        state.Phase = HandPhase.Dealing;
        state.PlayerCount = evt.PlayerCount;

        yield return BuildCommand("hand", new DealCards
        {
            HandId = evt.HandId,
            PlayerCount = evt.PlayerCount
        });
    }

    [ReactsTo(typeof(CardsDealt))]
    public IEnumerable<CommandBook> HandleCardsDealt(CardsDealt evt, HandFlowState state)
    {
        state.Phase = HandPhase.Blinds;
        yield return BuildCommand("hand", new PostBlinds { HandId = state.HandId });
    }

    [ReactsTo(typeof(HandComplete))]
    public IEnumerable<CommandBook> HandleHandComplete(HandComplete evt, HandFlowState state)
    {
        state.Phase = HandPhase.Complete;
        yield return BuildCommand("table", new EndHand
        {
            HandId = state.HandId,
            WinnerId = evt.WinnerId
        });
    }
}
// docs:end:pm_handler
