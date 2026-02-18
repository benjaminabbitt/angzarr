using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;
using Angzarr;
using Angzarr.Examples;

namespace HandFlow;

/// <summary>
/// Hand flow process manager.
/// Orchestrates the flow of poker hands by managing state machines
/// and sending commands to drive hands forward.
/// </summary>
public class HandFlowProcessManager
{
    private readonly Dictionary<string, HandProcess> _processes = new();

    public List<Cover> Prepare(EventBook trigger, EventBook processState)
    {
        var destinations = new List<Cover>();

        foreach (var page in trigger.Pages)
        {
            var typeUrl = page.Event.TypeUrl;
            if (typeUrl.Contains("HandStarted"))
            {
                var evt = page.Event.Unpack<HandStarted>();
                destinations.Add(new Cover
                {
                    Domain = "hand",
                    Root = new UUID { Value = evt.HandRoot }
                });
            }
        }

        return destinations;
    }

    public (List<CommandBook> Commands, EventBook? Events) Handle(
        EventBook trigger,
        EventBook processState,
        List<EventBook> destinations)
    {
        var commands = new List<CommandBook>();

        foreach (var page in trigger.Pages)
        {
            var eventAny = page.Event;
            var typeUrl = eventAny.TypeUrl;

            if (typeUrl.Contains("HandStarted"))
            {
                var evt = eventAny.Unpack<HandStarted>();
                var cmd = HandleHandStarted(evt);
                if (cmd != null) commands.Add(cmd);
            }
            else if (typeUrl.Contains("CardsDealt"))
            {
                var evt = eventAny.Unpack<CardsDealt>();
                var cmd = HandleCardsDealt(evt);
                if (cmd != null) commands.Add(cmd);
            }
            else if (typeUrl.Contains("BlindPosted"))
            {
                var evt = eventAny.Unpack<BlindPosted>();
                var cmd = HandleBlindPosted(evt);
                if (cmd != null) commands.Add(cmd);
            }
            else if (typeUrl.Contains("ActionTaken"))
            {
                var evt = eventAny.Unpack<ActionTaken>();
                var cmd = HandleActionTaken(evt);
                if (cmd != null) commands.Add(cmd);
            }
            else if (typeUrl.Contains("CommunityCardsDealt"))
            {
                var evt = eventAny.Unpack<CommunityCardsDealt>();
                var cmd = HandleCommunityCardsDealt(evt);
                if (cmd != null) commands.Add(cmd);
            }
            else if (typeUrl.Contains("PotAwarded"))
            {
                var evt = eventAny.Unpack<PotAwarded>();
                HandlePotAwarded(evt);
            }
        }

        return (commands, null);
    }

    private CommandBook? HandleHandStarted(HandStarted evt)
    {
        var handId = $"{Convert.ToHexString(evt.HandRoot.ToByteArray()).ToLowerInvariant()}_{evt.HandNumber}";

        var process = new HandProcess
        {
            HandId = handId,
            TableRoot = evt.HandRoot,
            HandNumber = evt.HandNumber,
            GameVariant = evt.GameVariant,
            DealerPosition = evt.DealerPosition,
            SmallBlindPosition = evt.SmallBlindPosition,
            BigBlindPosition = evt.BigBlindPosition,
            SmallBlind = evt.SmallBlind,
            BigBlind = evt.BigBlind,
            Phase = HandPhase.Dealing
        };

        foreach (var player in evt.ActivePlayers)
        {
            process.Players[player.Position] = new PlayerProcessState
            {
                PlayerRoot = player.PlayerRoot,
                Position = player.Position,
                Stack = player.Stack
            };
            process.ActivePositions.Add(player.Position);
        }

        process.ActivePositions.Sort();
        _processes[handId] = process;

        return null; // Wait for CardsDealt event
    }

    private CommandBook? HandleCardsDealt(CardsDealt evt)
    {
        var handId = $"{Convert.ToHexString(evt.TableRoot.ToByteArray()).ToLowerInvariant()}_{evt.HandNumber}";
        if (!_processes.TryGetValue(handId, out var process))
            return null;

        process.Phase = HandPhase.PostingBlinds;
        process.MinRaise = process.BigBlind;

        // Post small blind first
        var sbPlayer = process.Players.GetValueOrDefault(process.SmallBlindPosition);
        if (sbPlayer != null)
        {
            return BuildPostBlindCommand(process, sbPlayer, "small", process.SmallBlind);
        }

        return null;
    }

    private CommandBook? HandleBlindPosted(BlindPosted evt)
    {
        var process = FindProcessByPlayer(evt.PlayerRoot);
        if (process == null)
            return null;

        // Update player state
        foreach (var (_, player) in process.Players)
        {
            if (player.PlayerRoot.Equals(evt.PlayerRoot))
            {
                player.Stack = evt.PlayerStack;
                player.BetThisRound = evt.Amount;
                player.TotalInvested = evt.Amount;
                break;
            }
        }

        process.PotTotal = evt.PotTotal;

        if (evt.BlindType == "small")
        {
            process.SmallBlindPosted = true;
            process.CurrentBet = evt.Amount;

            // Post big blind
            var bbPlayer = process.Players.GetValueOrDefault(process.BigBlindPosition);
            if (bbPlayer != null)
            {
                return BuildPostBlindCommand(process, bbPlayer, "big", process.BigBlind);
            }
        }
        else if (evt.BlindType == "big")
        {
            process.BigBlindPosted = true;
            process.CurrentBet = evt.Amount;
            process.Phase = HandPhase.Betting;
            // Betting begins - would send RequestAction command
        }

        return null;
    }

    private CommandBook? HandleActionTaken(ActionTaken evt)
    {
        var process = FindProcessByPlayer(evt.PlayerRoot);
        if (process == null)
            return null;

        // Update player state
        foreach (var (pos, player) in process.Players)
        {
            if (player.PlayerRoot.Equals(evt.PlayerRoot))
            {
                player.Stack = evt.PlayerStack;
                player.HasActed = true;

                if (evt.Action == ActionType.Fold)
                    player.HasFolded = true;
                else if (evt.Action == ActionType.AllIn)
                {
                    player.IsAllIn = true;
                    player.BetThisRound += evt.Amount;
                    player.TotalInvested += evt.Amount;
                }
                else if (evt.Action is ActionType.Call or ActionType.Bet or ActionType.Raise)
                {
                    player.BetThisRound += evt.Amount;
                    player.TotalInvested += evt.Amount;
                }

                if (evt.Action is ActionType.Bet or ActionType.Raise or ActionType.AllIn)
                {
                    if (player.BetThisRound > process.CurrentBet)
                    {
                        var raiseAmount = player.BetThisRound - process.CurrentBet;
                        process.CurrentBet = player.BetThisRound;
                        process.MinRaise = Math.Max(process.MinRaise, raiseAmount);
                        process.LastAggressor = pos;
                    }
                }
                break;
            }
        }

        process.PotTotal = evt.PotTotal;

        // Check if betting round is complete
        if (IsBettingComplete(process))
        {
            return EndBettingRound(process);
        }

        return null;
    }

    private CommandBook? HandleCommunityCardsDealt(CommunityCardsDealt evt)
    {
        var process = _processes.Values.FirstOrDefault(p =>
            p.CommunityCardCount + evt.Cards.Count == evt.AllCommunityCards.Count);
        if (process == null)
            return null;

        process.CommunityCardCount = evt.AllCommunityCards.Count;
        process.BettingPhase = evt.Phase;
        process.Phase = HandPhase.Betting;

        // Reset betting for new round
        foreach (var player in process.Players.Values)
        {
            player.BetThisRound = 0;
            player.HasActed = false;
        }
        process.CurrentBet = 0;

        return null;
    }

    private void HandlePotAwarded(PotAwarded evt)
    {
        // Find and mark process as complete
        var process = _processes.Values.FirstOrDefault(p => p.Phase != HandPhase.Complete);
        if (process != null)
        {
            process.Phase = HandPhase.Complete;
        }
    }

    private bool IsBettingComplete(HandProcess process)
    {
        var activePlayers = process.Players.Values
            .Where(p => !p.HasFolded && !p.IsAllIn)
            .ToList();

        if (activePlayers.Count <= 1)
            return true;

        foreach (var player in activePlayers)
        {
            if (!player.HasActed)
                return false;
            if (player.BetThisRound < process.CurrentBet && !player.IsAllIn)
                return false;
        }

        return true;
    }

    private CommandBook? EndBettingRound(HandProcess process)
    {
        var playersInHand = process.Players.Values.Where(p => !p.HasFolded).ToList();
        var activePlayers = playersInHand.Where(p => !p.IsAllIn).ToList();

        // If only one player left, award pot
        if (playersInHand.Count == 1)
        {
            return BuildAwardPotCommand(process, playersInHand);
        }

        // Determine next phase based on game variant
        if (process.GameVariant is GameVariant.TexasHoldem or GameVariant.Omaha)
        {
            return AdvanceHoldemPhase(process);
        }

        return null;
    }

    private CommandBook? AdvanceHoldemPhase(HandProcess process)
    {
        int cardsToDealt = process.BettingPhase switch
        {
            BettingPhase.Preflop => 3, // Flop
            BettingPhase.Flop => 1,     // Turn
            BettingPhase.Turn => 1,     // River
            BettingPhase.River => 0,    // Showdown
            _ => 0
        };

        if (cardsToDealt > 0)
        {
            process.Phase = HandPhase.DealingCommunity;
            return BuildDealCommunityCommand(process, cardsToDealt);
        }

        // Showdown - auto-award pot (simplified)
        var playersInHand = process.Players.Values.Where(p => !p.HasFolded).ToList();
        return BuildAwardPotCommand(process, playersInHand);
    }

    private CommandBook BuildPostBlindCommand(HandProcess process, PlayerProcessState player, string blindType, long amount)
    {
        var cmd = new PostBlind
        {
            PlayerRoot = player.PlayerRoot,
            BlindType = blindType,
            Amount = amount
        };

        var cmdAny = Any.Pack(cmd, "type.googleapis.com/");
        var handRoot = ByteString.CopyFrom(Convert.FromHexString(process.HandId.Split('_')[0]));

        return new CommandBook
        {
            Cover = new Cover
            {
                Domain = "hand",
                Root = new UUID { Value = handRoot }
            },
            Pages = { new CommandPage { Command = cmdAny } }
        };
    }

    private CommandBook BuildDealCommunityCommand(HandProcess process, int count)
    {
        var cmd = new DealCommunityCards { Count = count };
        var cmdAny = Any.Pack(cmd, "type.googleapis.com/");
        var handRoot = ByteString.CopyFrom(Convert.FromHexString(process.HandId.Split('_')[0]));

        return new CommandBook
        {
            Cover = new Cover
            {
                Domain = "hand",
                Root = new UUID { Value = handRoot }
            },
            Pages = { new CommandPage { Command = cmdAny } }
        };
    }

    private CommandBook BuildAwardPotCommand(HandProcess process, List<PlayerProcessState> winners)
    {
        process.Phase = HandPhase.Complete;

        var split = process.PotTotal / winners.Count;
        var remainder = process.PotTotal % winners.Count;

        var awards = winners.Select((w, i) => new PotAward
        {
            PlayerRoot = w.PlayerRoot,
            Amount = split + (i < remainder ? 1 : 0),
            PotType = "main"
        }).ToList();

        var cmd = new AwardPot();
        cmd.Awards.AddRange(awards);

        var cmdAny = Any.Pack(cmd, "type.googleapis.com/");
        var handRoot = ByteString.CopyFrom(Convert.FromHexString(process.HandId.Split('_')[0]));

        return new CommandBook
        {
            Cover = new Cover
            {
                Domain = "hand",
                Root = new UUID { Value = handRoot }
            },
            Pages = { new CommandPage { Command = cmdAny } }
        };
    }

    private HandProcess? FindProcessByPlayer(ByteString playerRoot)
    {
        return _processes.Values.FirstOrDefault(p =>
            p.Players.Values.Any(pl => pl.PlayerRoot.Equals(playerRoot)));
    }
}
