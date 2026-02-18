using Angzarr;
using Angzarr.Client;
using Angzarr.Examples;
using Google.Protobuf.WellKnownTypes;

namespace HandFlowOO;

/// <summary>
/// PM's aggregate state (rebuilt from its own events).
/// For simplicity in this example, we use a minimal state.
/// </summary>
public class PMState
{
    public byte[]? HandRoot { get; set; }
    public bool HandInProgress { get; set; }
}

/// <summary>
/// Hand Flow Process Manager using OO-style attributes.
///
/// This PM orchestrates poker hand flow by:
/// - Tracking when hands start and complete
/// - Coordinating between table and hand domains
/// </summary>
public class HandFlowPM : ProcessManager<PMState>
{
    public override string Name => "hand-flow";

    public HandFlowPM() : base() { }

    public HandFlowPM(EventBook? processState) : base(processState) { }

    protected override PMState CreateEmptyState() => new();

    protected override void ApplyEvent(PMState state, Any eventAny)
    {
        // In this simplified example, we don't persist PM events.
    }

    /// <summary>
    /// Declare the hand destination needed when a hand starts.
    /// </summary>
    [Prepares(typeof(HandStarted))]
    public List<Cover> PrepareHandStarted(HandStarted evt)
    {
        return new List<Cover>
        {
            new Cover
            {
                Domain = "hand",
                Root = new Angzarr.UUID { Value = evt.HandRoot }
            }
        };
    }

    /// <summary>
    /// Process the HandStarted event.
    ///
    /// Initialize hand process (not persisted in this simplified version).
    /// The saga-table-hand will send DealCards, so we don't emit commands here.
    /// </summary>
    [ReactsTo(typeof(HandStarted), InputDomain = "table")]
    public List<CommandBook> HandleHandStarted(HandStarted evt, List<EventBook> destinations)
    {
        return new List<CommandBook>();
    }

    /// <summary>
    /// Process the CardsDealt event.
    ///
    /// Post small blind command. In a real implementation, we'd track state
    /// to know which blind to post.
    /// </summary>
    [ReactsTo(typeof(CardsDealt), InputDomain = "hand")]
    public List<CommandBook> HandleCardsDealt(CardsDealt evt, List<EventBook> destinations)
    {
        return new List<CommandBook>();
    }

    /// <summary>
    /// Process the BlindPosted event.
    ///
    /// In a full implementation, we'd check if both blinds are posted
    /// and then start the betting round.
    /// </summary>
    [ReactsTo(typeof(BlindPosted), InputDomain = "hand")]
    public List<CommandBook> HandleBlindPosted(BlindPosted evt, List<EventBook> destinations)
    {
        return new List<CommandBook>();
    }

    /// <summary>
    /// Process the ActionTaken event.
    ///
    /// In a full implementation, we'd check if betting is complete
    /// and advance to the next phase.
    /// </summary>
    [ReactsTo(typeof(ActionTaken), InputDomain = "hand")]
    public List<CommandBook> HandleActionTaken(ActionTaken evt, List<EventBook> destinations)
    {
        return new List<CommandBook>();
    }

    /// <summary>
    /// Process the CommunityCardsDealt event.
    ///
    /// Start new betting round after community cards.
    /// </summary>
    [ReactsTo(typeof(CommunityCardsDealt), InputDomain = "hand")]
    public List<CommandBook> HandleCommunityDealt(CommunityCardsDealt evt, List<EventBook> destinations)
    {
        return new List<CommandBook>();
    }

    /// <summary>
    /// Process the PotAwarded event.
    ///
    /// Hand is complete. Clean up.
    /// </summary>
    [ReactsTo(typeof(PotAwarded), InputDomain = "hand")]
    public List<CommandBook> HandlePotAwarded(PotAwarded evt, List<EventBook> destinations)
    {
        return new List<CommandBook>();
    }
}
