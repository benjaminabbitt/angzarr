using Google.Protobuf;

namespace HandFlow;

/// <summary>
/// Tracks a player's state within the process manager.
/// </summary>
public class PlayerProcessState
{
    public ByteString PlayerRoot { get; set; } = ByteString.Empty;
    public int Position { get; set; }
    public long Stack { get; set; }
    public long BetThisRound { get; set; }
    public long TotalInvested { get; set; }
    public bool HasActed { get; set; }
    public bool HasFolded { get; set; }
    public bool IsAllIn { get; set; }
}
