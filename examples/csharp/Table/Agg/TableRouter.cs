using Google.Protobuf;
using Angzarr;
using Angzarr.Client;
using Angzarr.Examples;
using Table.Agg.Handlers;

namespace Table.Agg;

/// <summary>
/// Functional router for Table aggregate.
/// </summary>
public static class TableRouter
{
    public static CommandRouter Create()
    {
        return new CommandRouter("table", eb => TableState.FromEventBook(eb))
            .On<CreateTable>((cmd, state) => CreateHandler.Handle(cmd, (TableState)state))
            .On<JoinTable>((cmd, state) => JoinHandler.Handle(cmd, (TableState)state))
            .On<LeaveTable>((cmd, state) => LeaveHandler.Handle(cmd, (TableState)state))
            .On<StartHand>((cmd, state) => StartHandHandler.Handle(cmd, (TableState)state))
            .On<EndHand>((cmd, state) => EndHandHandler.Handle(cmd, (TableState)state));
    }
}
