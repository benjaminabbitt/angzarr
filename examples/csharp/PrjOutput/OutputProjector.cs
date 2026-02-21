using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;
using Angzarr;
using Angzarr.Examples;

namespace PrjOutput;

/// <summary>
/// Projector that renders events as human-readable text output.
/// </summary>
public class OutputProjector
{
    private readonly TextRenderer _renderer = new();
    private readonly Action<string> _outputFn;
    private readonly bool _showTimestamps;

    public OutputProjector(Action<string>? outputFn = null, bool showTimestamps = false)
    {
        _outputFn = outputFn ?? Console.WriteLine;
        _showTimestamps = showTimestamps;
    }

    public void SetPlayerName(ByteString playerRoot, string name)
    {
        _renderer.SetPlayerName(playerRoot, name);
    }

    public void HandleEventPage(EventPage page)
    {
        var eventAny = page.Event;
        var typeUrl = eventAny.TypeUrl;
        var eventType = typeUrl.Contains('.') ? typeUrl.Split('.').Last() : typeUrl;

        var evt = UnpackEvent(eventType, eventAny);
        if (evt == null)
        {
            _outputFn($"[Unknown event type: {typeUrl}]");
            return;
        }

        var text = _renderer.Render(eventType, evt);
        if (!string.IsNullOrEmpty(text))
        {
            if (_showTimestamps && page.CreatedAt != null)
            {
                var ts = page.CreatedAt.ToDateTime();
                text = $"[{ts:HH:mm:ss}] {text}";
            }
            _outputFn(text);
        }
    }

    public void HandleEventBook(EventBook eventBook)
    {
        foreach (var page in eventBook.Pages)
        {
            HandleEventPage(page);
        }
    }

    public Projection Handle(EventBook eventBook)
    {
        HandleEventBook(eventBook);

        var seq = eventBook.Pages.LastOrDefault()?.Sequence ?? 0;

        return new Projection
        {
            Cover = eventBook.Cover,
            Projector = "output",
            Sequence = seq
        };
    }

    private static object? UnpackEvent(string eventType, Any eventAny)
    {
        return eventType switch
        {
            "PlayerRegistered" => eventAny.Unpack<PlayerRegistered>(),
            "FundsDeposited" => eventAny.Unpack<FundsDeposited>(),
            "FundsWithdrawn" => eventAny.Unpack<FundsWithdrawn>(),
            "FundsReserved" => eventAny.Unpack<FundsReserved>(),
            "FundsReleased" => eventAny.Unpack<FundsReleased>(),
            "TableCreated" => eventAny.Unpack<TableCreated>(),
            "PlayerJoined" => eventAny.Unpack<PlayerJoined>(),
            "PlayerLeft" => eventAny.Unpack<PlayerLeft>(),
            "HandStarted" => eventAny.Unpack<HandStarted>(),
            "HandEnded" => eventAny.Unpack<HandEnded>(),
            "CardsDealt" => eventAny.Unpack<CardsDealt>(),
            "BlindPosted" => eventAny.Unpack<BlindPosted>(),
            "ActionTaken" => eventAny.Unpack<ActionTaken>(),
            "CommunityCardsDealt" => eventAny.Unpack<CommunityCardsDealt>(),
            "PotAwarded" => eventAny.Unpack<PotAwarded>(),
            "HandComplete" => eventAny.Unpack<HandComplete>(),
            _ => null
        };
    }
}
