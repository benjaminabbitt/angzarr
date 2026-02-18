using System.Security.Cryptography;
using System.Text;
using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;
using Angzarr.Client;
using Angzarr.Examples;

namespace Table.Agg.Handlers;

/// <summary>
/// Handler for StartHand command.
/// </summary>
public static class StartHandHandler
{
    public static HandStarted Handle(StartHand cmd, TableState state)
    {
        // Guard
        if (!state.Exists)
            throw CommandRejectedError.PreconditionFailed("Table does not exist");
        if (state.Status == "in_hand")
            throw CommandRejectedError.PreconditionFailed("Hand already in progress");
        if (state.ActivePlayerCount < 2)
            throw CommandRejectedError.PreconditionFailed("Not enough players to start hand");

        // Compute
        var handNumber = state.HandCount + 1;
        var handRoot = GenerateHandRoot(state.TableId, handNumber);
        var dealerPosition = state.NextDealerPosition();

        var activePositions = state.Seats.Values
            .Where(s => !s.IsSittingOut)
            .Select(s => s.Position)
            .OrderBy(p => p)
            .ToList();

        var dealerIdx = activePositions.IndexOf(dealerPosition);
        if (dealerIdx < 0) dealerIdx = 0;

        int sbPosition, bbPosition;
        if (activePositions.Count == 2)
        {
            sbPosition = activePositions[dealerIdx];
            bbPosition = activePositions[(dealerIdx + 1) % 2];
        }
        else
        {
            sbPosition = activePositions[(dealerIdx + 1) % activePositions.Count];
            bbPosition = activePositions[(dealerIdx + 2) % activePositions.Count];
        }

        var activePlayers = activePositions.Select(pos =>
        {
            var seat = state.Seats[pos];
            return new SeatSnapshot
            {
                Position = pos,
                PlayerRoot = seat.PlayerRoot,
                Stack = seat.Stack
            };
        }).ToList();

        var evt = new HandStarted
        {
            HandRoot = ByteString.CopyFrom(handRoot),
            HandNumber = handNumber,
            DealerPosition = dealerPosition,
            SmallBlindPosition = sbPosition,
            BigBlindPosition = bbPosition,
            GameVariant = state.GameVariant,
            SmallBlind = state.SmallBlind,
            BigBlind = state.BigBlind,
            StartedAt = Timestamp.FromDateTime(DateTime.UtcNow)
        };
        evt.ActivePlayers.AddRange(activePlayers);

        return evt;
    }

    private static byte[] GenerateHandRoot(string tableId, long handNumber)
    {
        using var sha = SHA256.Create();
        var input = $"angzarr.poker.hand.{tableId}.{handNumber}";
        var hash = sha.ComputeHash(Encoding.UTF8.GetBytes(input));
        return hash.Take(16).ToArray();
    }
}
