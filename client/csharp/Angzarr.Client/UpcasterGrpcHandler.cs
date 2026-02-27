using Grpc.Core;

namespace Angzarr.Client;

/// <summary>
/// gRPC service handler for upcaster.
///
/// Wraps an UpcasterRouter and implements the gRPC UpcasterService.
/// This can be used directly with ASP.NET Core's gRPC services or
/// with a standalone gRPC server.
///
/// ASP.NET Core Usage:
/// <code>
/// public class PlayerUpcasterService : UpcasterGrpcHandler
/// {
///     public PlayerUpcasterService() : base(
///         new UpcasterRouter("player")
///             .On("PlayerRegisteredV1", old =>
///             {
///                 var v1 = old.Unpack&lt;PlayerRegisteredV1&gt;();
///                 return Any.Pack(new PlayerRegistered
///                 {
///                     DisplayName = v1.DisplayName
///                 }, "type.googleapis.com/");
///             }))
///     {
///     }
/// }
///
/// // In Program.cs:
/// builder.Services.AddGrpc();
/// app.MapGrpcService&lt;PlayerUpcasterService&gt;();
/// </code>
///
/// Standalone Server Usage:
/// <code>
/// var router = new UpcasterRouter("player")
///     .On("PlayerRegisteredV1", transformer);
///
/// var server = new Server
/// {
///     Services = { UpcasterService.BindService(new UpcasterGrpcHandler(router)) },
///     Ports = { new ServerPort("localhost", 50401, ServerCredentials.Insecure) }
/// };
/// server.Start();
/// </code>
/// </summary>
public class UpcasterGrpcHandler : Angzarr.UpcasterService.UpcasterServiceBase
{
    private readonly UpcasterRouter _router;

    /// <summary>
    /// Create a new upcaster gRPC handler.
    /// </summary>
    /// <param name="router">The upcaster router to use for transformations.</param>
    public UpcasterGrpcHandler(UpcasterRouter router)
    {
        _router = router;
    }

    /// <summary>
    /// Get the underlying router.
    /// </summary>
    public UpcasterRouter Router => _router;

    /// <summary>
    /// Get the domain this handler serves.
    /// </summary>
    public string Domain => _router.Domain;

    /// <summary>
    /// Transform events to current versions.
    /// </summary>
    public override Task<UpcastResponse> Upcast(UpcastRequest request, ServerCallContext context)
    {
        var events = _router.Upcast(request.Events);
        var response = new UpcastResponse();
        response.Events.AddRange(events);
        return Task.FromResult(response);
    }
}
