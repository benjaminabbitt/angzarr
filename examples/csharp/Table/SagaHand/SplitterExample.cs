using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;
using Angzarr;
using Angzarr.Client;
using Angzarr.Examples;

namespace Angzarr.Examples.Table.SagaHand;

/// <summary>
/// Saga splitter pattern example for documentation.
///
/// Demonstrates the splitter pattern where one event triggers commands
/// to multiple different aggregates.
/// </summary>

// docs:start:saga_splitter
public static class SplitterExample
{
    public static IEnumerable<CommandBook> HandleTableSettled(TableSettled @event, SagaContext ctx)
    {
        // Split one event into commands for multiple player aggregates
        foreach (var payout in @event.Payouts)
        {
            var cmd = new TransferFunds
            {
                TableRoot = @event.TableRoot,
                Amount = payout.Amount
            };

            var targetSeq = ctx.GetSequence("player", payout.PlayerRoot);

            yield return new CommandBook
            {
                Cover = new Cover
                {
                    Domain = "player",
                    Root = new UUID { Value = payout.PlayerRoot }
                },
                Pages = {
                    new CommandPage
                    {
                        Num = (uint)targetSeq,
                        Command = Any.Pack(cmd)
                    }
                }
            };
        }
        // One CommandBook per player
    }
}
// docs:end:saga_splitter
