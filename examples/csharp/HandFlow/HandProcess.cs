using Google.Protobuf;
using Angzarr.Examples;

namespace HandFlow;

/// <summary>
/// Process manager state for a single hand.
/// Tracks orchestration state separately from domain state.
/// </summary>
public class HandProcess
{
    public string HandId { get; set; } = "";
    public ByteString TableRoot { get; set; } = ByteString.Empty;
    public long HandNumber { get; set; }
    public GameVariant GameVariant { get; set; } = GameVariant.Unspecified;

    // State machine
    public HandPhase Phase { get; set; } = HandPhase.WaitingForStart;
    public BettingPhase BettingPhase { get; set; } = BettingPhase.Preflop;

    // Player tracking
    public Dictionary<int, PlayerProcessState> Players { get; } = new();
    public List<int> ActivePositions { get; } = new();

    // Position tracking
    public int DealerPosition { get; set; }
    public int SmallBlindPosition { get; set; }
    public int BigBlindPosition { get; set; }
    public int ActionOn { get; set; } = -1;
    public int LastAggressor { get; set; } = -1;

    // Betting state
    public long SmallBlind { get; set; }
    public long BigBlind { get; set; }
    public long CurrentBet { get; set; }
    public long MinRaise { get; set; }
    public long PotTotal { get; set; }

    // Blind posting progress
    public bool SmallBlindPosted { get; set; }
    public bool BigBlindPosted { get; set; }

    // Timeout handling
    public int ActionTimeoutSeconds { get; set; } = 30;
    public DateTime? ActionStartedAt { get; set; }

    // Community cards
    public int CommunityCardCount { get; set; }
}
