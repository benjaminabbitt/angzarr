using Angzarr;

namespace Angzarr.Examples.Projector;

public interface IReceiptProjector
{
    Projection? Project(EventBook eventBook);
}
