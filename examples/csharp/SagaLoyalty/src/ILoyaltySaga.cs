using Angzarr;

namespace Angzarr.Examples.Saga;

public interface ILoyaltySaga
{
    IReadOnlyList<CommandBook> ProcessEvents(EventBook eventBook);
}
