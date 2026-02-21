using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;
using Angzarr;
using Hand.Agg;

namespace Tests.Support;

/// <summary>
/// Shared test context for step definitions.
/// </summary>
public class TestContext
{
    public HandAggregate? HandAggregate { get; set; }
    public EventBook HandEventBook { get; set; } = new();
    public Exception? LastException { get; set; }
    public IMessage? LastEvent { get; set; }

    public void AddHandEvent(IMessage evt)
    {
        var any = Any.Pack(evt, "type.googleapis.com/");
        HandEventBook.Pages.Add(new EventPage
        {
            Sequence = (uint)HandEventBook.Pages.Count,
            Event = any
        });
    }
}
