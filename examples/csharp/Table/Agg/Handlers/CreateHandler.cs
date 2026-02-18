using Google.Protobuf.WellKnownTypes;
using Angzarr.Client;
using Angzarr.Examples;

namespace Table.Agg.Handlers;

/// <summary>
/// Handler for CreateTable command.
/// </summary>
public static class CreateHandler
{
    public static TableCreated Handle(CreateTable cmd, TableState state)
    {
        // Guard
        if (state.Exists)
            throw CommandRejectedError.PreconditionFailed("Table already exists");

        // Validate
        if (string.IsNullOrEmpty(cmd.TableName))
            throw CommandRejectedError.InvalidArgument("table_name is required");
        if (cmd.SmallBlind <= 0)
            throw CommandRejectedError.InvalidArgument("small_blind must be positive");
        if (cmd.BigBlind <= 0)
            throw CommandRejectedError.InvalidArgument("big_blind must be positive");
        if (cmd.BigBlind < cmd.SmallBlind)
            throw CommandRejectedError.InvalidArgument("big_blind must be >= small_blind");
        if (cmd.MaxPlayers < 2 || cmd.MaxPlayers > 10)
            throw CommandRejectedError.InvalidArgument("max_players must be between 2 and 10");

        // Compute
        return new TableCreated
        {
            TableName = cmd.TableName,
            GameVariant = cmd.GameVariant,
            SmallBlind = cmd.SmallBlind,
            BigBlind = cmd.BigBlind,
            MinBuyIn = cmd.MinBuyIn != 0 ? cmd.MinBuyIn : cmd.BigBlind * 20,
            MaxBuyIn = cmd.MaxBuyIn != 0 ? cmd.MaxBuyIn : cmd.BigBlind * 100,
            MaxPlayers = cmd.MaxPlayers != 0 ? cmd.MaxPlayers : 9,
            ActionTimeoutSeconds = cmd.ActionTimeoutSeconds != 0 ? cmd.ActionTimeoutSeconds : 30,
            CreatedAt = Timestamp.FromDateTime(DateTime.UtcNow)
        };
    }
}
