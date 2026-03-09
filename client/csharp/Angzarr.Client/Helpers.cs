using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;

namespace Angzarr.Client;

/// <summary>
/// Helper methods for working with Angzarr types.
/// </summary>
public static class Helpers
{
    /// <summary>
    /// Convert a System.Guid to an Angzarr UUID proto.
    /// </summary>
    public static Angzarr.UUID UuidToProto(Guid guid)
    {
        return new Angzarr.UUID { Value = ByteString.CopyFrom(guid.ToByteArray()) };
    }

    /// <summary>
    /// Convert an Angzarr UUID proto to a System.Guid.
    /// </summary>
    public static Guid ProtoToUuid(Angzarr.UUID uuid)
    {
        return new Guid(uuid.Value.ToByteArray());
    }

    /// <summary>
    /// Get the domain from an EventBook.
    /// </summary>
    public static string Domain(Angzarr.EventBook book)
    {
        return book.Cover?.Domain ?? "";
    }

    /// <summary>
    /// Get the correlation ID from an EventBook.
    /// </summary>
    public static string CorrelationId(Angzarr.EventBook book)
    {
        return book.Cover?.CorrelationId ?? "";
    }

    /// <summary>
    /// Check if an EventBook has a correlation ID.
    /// </summary>
    public static bool HasCorrelationId(Angzarr.EventBook book)
    {
        return !string.IsNullOrEmpty(book.Cover?.CorrelationId);
    }

    /// <summary>
    /// Get the root UUID from an EventBook.
    /// </summary>
    public static Angzarr.UUID? RootUuid(Angzarr.EventBook book)
    {
        return book.Cover?.Root;
    }

    /// <summary>
    /// Get the root UUID as hex string from an EventBook.
    /// </summary>
    public static string RootIdHex(Angzarr.EventBook book)
    {
        var root = book.Cover?.Root;
        if (root == null)
            return "";
        return Convert.ToHexString(root.Value.ToByteArray()).ToLowerInvariant();
    }

    /// <summary>
    /// Get the edition from an EventBook.
    /// </summary>
    public static Angzarr.Edition? Edition(Angzarr.EventBook book)
    {
        return book.Cover?.Edition;
    }

    /// <summary>
    /// Calculate the next sequence number from an EventBook.
    /// Uses the framework-computed next_sequence field.
    /// </summary>
    public static uint NextSequence(Angzarr.EventBook? book)
    {
        if (book == null)
            return 1;
        return book.NextSequence > 0 ? book.NextSequence : 1;
    }

    /// <summary>
    /// Get the sequence number from an EventPage's header.
    /// Returns 0 if header or sequence is not set.
    /// </summary>
    public static uint SequenceNum(Angzarr.EventPage page)
    {
        return page.Header?.Sequence ?? 0;
    }

    /// <summary>
    /// Get the sequence number from a CommandPage's header.
    /// Returns 0 if header or sequence is not set.
    /// </summary>
    public static uint SequenceNum(Angzarr.CommandPage page)
    {
        return page.Header?.Sequence ?? 0;
    }

    /// <summary>
    /// Set the sequence number on an EventPage's header.
    /// Creates the header if it doesn't exist.
    /// </summary>
    public static void SetSequence(Angzarr.EventPage page, uint sequence)
    {
        page.Header ??= new Angzarr.PageHeader();
        page.Header.Sequence = sequence;
    }

    /// <summary>
    /// Set the sequence number on a CommandPage's header.
    /// Creates the header if it doesn't exist.
    /// </summary>
    public static void SetSequence(Angzarr.CommandPage page, uint sequence)
    {
        page.Header ??= new Angzarr.PageHeader();
        page.Header.Sequence = sequence;
    }

    /// <summary>
    /// Get the type URL for a protobuf message.
    /// </summary>
    public static string TypeUrl(IMessage message)
    {
        return "type.googleapis.com/" + message.Descriptor.FullName;
    }

    /// <summary>
    /// Extract the type name from a type URL.
    /// </summary>
    public static string TypeNameFromUrl(string typeUrl)
    {
        var idx = typeUrl.LastIndexOf('/');
        return idx >= 0 ? typeUrl[(idx + 1)..] : typeUrl;
    }

    private const string TypeUrlPrefix = "type.googleapis.com/";

    /// <summary>
    /// Check if a type URL matches the given fully qualified type name.
    /// </summary>
    /// <param name="typeUrl">Full type URL (e.g., "type.googleapis.com/examples.CardsDealt")</param>
    /// <param name="typeName">Fully qualified type name (e.g., "examples.CardsDealt")</param>
    /// <returns>true if typeUrl equals TypeUrlPrefix + typeName</returns>
    public static bool TypeUrlMatches(string typeUrl, string typeName)
    {
        return typeUrl == TypeUrlPrefix + typeName;
    }

    /// <summary>
    /// Get the current timestamp as a protobuf Timestamp.
    /// </summary>
    public static Timestamp Now()
    {
        return Timestamp.FromDateTime(DateTime.UtcNow);
    }

    /// <summary>
    /// Parse a timestamp string to a protobuf Timestamp.
    /// </summary>
    public static Timestamp ParseTimestamp(string value)
    {
        if (DateTime.TryParse(value, out var dt))
        {
            return Timestamp.FromDateTime(dt.ToUniversalTime());
        }
        throw new InvalidTimestampError($"Cannot parse timestamp: {value}");
    }

    /// <summary>
    /// Pack a protobuf message into an Any.
    /// </summary>
    public static Any PackAny(IMessage message)
    {
        return Any.Pack(message, "type.googleapis.com/");
    }

    /// <summary>
    /// Pack an event into an EventPage.
    /// </summary>
    public static Angzarr.EventPage PackEvent(IMessage eventMessage)
    {
        return new Angzarr.EventPage { Event = PackAny(eventMessage) };
    }

    /// <summary>
    /// Pack multiple events into EventPages.
    /// </summary>
    public static IEnumerable<Angzarr.EventPage> PackEvents(params IMessage[] events)
    {
        return events.Select(PackEvent);
    }

    /// <summary>
    /// Create a new EventBook with the given events.
    /// </summary>
    public static Angzarr.EventBook NewEventBook(params IMessage[] events)
    {
        var book = new Angzarr.EventBook();
        book.Pages.AddRange(PackEvents(events));
        return book;
    }

    /// <summary>
    /// Create a new EventBook with multiple events.
    /// </summary>
    public static Angzarr.EventBook NewEventBookMulti(IEnumerable<IMessage> events)
    {
        var book = new Angzarr.EventBook();
        book.Pages.AddRange(events.Select(PackEvent));
        return book;
    }
}
