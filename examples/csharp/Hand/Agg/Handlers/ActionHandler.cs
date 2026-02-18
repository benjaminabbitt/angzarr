using Google.Protobuf.WellKnownTypes;
using Angzarr.Client;
using Angzarr.Examples;

namespace Hand.Agg.Handlers;

/// <summary>
/// Handler for PlayerAction command.
/// </summary>
public static class ActionHandler
{
    public static ActionTaken Handle(PlayerAction cmd, HandState state)
    {
        // Guard
        if (!state.Exists)
            throw CommandRejectedError.PreconditionFailed("Hand not dealt");
        if (state.Status != "betting")
            throw CommandRejectedError.PreconditionFailed("Not in betting phase");

        // Validate
        if (cmd.PlayerRoot.IsEmpty)
            throw CommandRejectedError.InvalidArgument("player_root is required");

        var player = state.GetPlayer(cmd.PlayerRoot);
        if (player == null)
            throw CommandRejectedError.PreconditionFailed("Player not in hand");
        if (player.HasFolded)
            throw CommandRejectedError.PreconditionFailed("Player has folded");
        if (player.IsAllIn)
            throw CommandRejectedError.PreconditionFailed("Player is all-in");

        var action = cmd.Action;
        var amount = cmd.Amount;
        var callAmount = state.CurrentBet - player.BetThisRound;

        switch (action)
        {
            case ActionType.Fold:
                amount = 0;
                break;
            case ActionType.Check:
                if (callAmount > 0)
                    throw CommandRejectedError.PreconditionFailed("Cannot check when there is a bet to call");
                amount = 0;
                break;
            case ActionType.Call:
                if (callAmount == 0)
                    throw CommandRejectedError.PreconditionFailed("Nothing to call");
                amount = Math.Min(callAmount, player.Stack);
                if (player.Stack - amount == 0)
                    action = ActionType.AllIn;
                break;
            case ActionType.Bet:
                if (state.CurrentBet > 0)
                    throw CommandRejectedError.PreconditionFailed("Cannot bet when there is already a bet");
                if (amount < state.BigBlind)
                    throw CommandRejectedError.InvalidArgument($"Bet must be at least {state.BigBlind}");
                if (amount > player.Stack)
                    throw CommandRejectedError.InvalidArgument("Bet exceeds stack");
                if (player.Stack - amount == 0)
                    action = ActionType.AllIn;
                break;
            case ActionType.Raise:
                if (state.CurrentBet == 0)
                    throw CommandRejectedError.PreconditionFailed("Cannot raise when there is no bet");
                var totalBet = player.BetThisRound + amount;
                var raiseAmount = totalBet - state.CurrentBet;
                if (raiseAmount < state.MinRaise && amount < player.Stack)
                    throw CommandRejectedError.InvalidArgument($"Raise must be at least {state.MinRaise}");
                if (amount > player.Stack)
                    throw CommandRejectedError.InvalidArgument("Raise exceeds stack");
                if (player.Stack - amount == 0)
                    action = ActionType.AllIn;
                break;
            case ActionType.AllIn:
                amount = player.Stack;
                break;
            default:
                throw CommandRejectedError.InvalidArgument("Invalid action");
        }

        // Compute
        var newStack = player.Stack - amount;
        var newPotTotal = state.GetPotTotal() + amount;

        return new ActionTaken
        {
            PlayerRoot = cmd.PlayerRoot,
            Action = action,
            Amount = amount,
            PlayerStack = newStack,
            PotTotal = newPotTotal,
            AmountToCall = Math.Max(state.CurrentBet, player.BetThisRound + amount) - player.BetThisRound,
            ActionAt = Timestamp.FromDateTime(DateTime.UtcNow)
        };
    }
}
