using Google.Protobuf;
using Angzarr;
using Angzarr.Client;
using Angzarr.Examples;
using Hand.Agg.Handlers;

namespace Hand.Agg;

/// <summary>
/// Functional router for Hand aggregate.
/// </summary>
public static class HandRouter
{
    public static CommandRouter Create()
    {
        return new CommandRouter("hand", eb => HandState.FromEventBook(eb))
            .On<DealCards>((cmd, state) => DealHandler.Handle(cmd, (HandState)state))
            .On<PostBlind>((cmd, state) => PostBlindHandler.Handle(cmd, (HandState)state))
            .On<PlayerAction>((cmd, state) => ActionHandler.Handle(cmd, (HandState)state))
            .On<DealCommunityCards>((cmd, state) => DealCommunityHandler.Handle(cmd, (HandState)state))
            .On<AwardPot>((cmd, state) => AwardPotHandler.Handle(cmd, (HandState)state));
    }
}
