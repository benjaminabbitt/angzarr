using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;
using Angzarr.Client;
using Angzarr.Examples;

namespace Hand.Agg.Handlers;

/// <summary>
/// Handler for AwardPot command.
/// </summary>
public static class AwardPotHandler
{
    public static HandComplete Handle(AwardPot cmd, HandState state)
    {
        // Guard
        if (!state.Exists)
            throw CommandRejectedError.PreconditionFailed("Hand not dealt");
        if (state.Status == "complete")
            throw CommandRejectedError.PreconditionFailed("Hand already complete");

        // Validate
        if (cmd.Awards.Count == 0)
            throw CommandRejectedError.InvalidArgument("No awards specified");

        foreach (var award in cmd.Awards)
        {
            var player = state.GetPlayer(award.PlayerRoot);
            if (player == null)
                throw CommandRejectedError.PreconditionFailed("Winner not in hand");
            if (player.HasFolded)
                throw CommandRejectedError.PreconditionFailed("Folded player cannot win pot");
        }

        // Compute
        var winners = cmd.Awards.Select(a => new PotWinner
        {
            PlayerRoot = a.PlayerRoot,
            Amount = a.Amount,
            PotType = a.PotType
        }).ToList();

        var finalStacks = state.Players.Values.Select(p =>
        {
            var winAmount = cmd.Awards.Where(a => a.PlayerRoot.Equals(p.PlayerRoot)).Sum(a => a.Amount);
            return new PlayerStackSnapshot
            {
                PlayerRoot = p.PlayerRoot,
                Stack = p.Stack + winAmount,
                IsAllIn = p.IsAllIn,
                HasFolded = p.HasFolded
            };
        }).ToList();

        var evt = new HandComplete
        {
            TableRoot = state.TableRoot,
            HandNumber = state.HandNumber,
            CompletedAt = Timestamp.FromDateTime(DateTime.UtcNow)
        };
        evt.Winners.AddRange(winners);
        evt.FinalStacks.AddRange(finalStacks);

        return evt;
    }
}
