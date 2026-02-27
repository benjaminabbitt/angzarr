using Angzarr.Client;
using Microsoft.AspNetCore.Builder;
using Microsoft.AspNetCore.Hosting;
using Microsoft.Extensions.DependencyInjection;

namespace Player.Upc;

/// <summary>
/// Player domain upcaster gRPC server.
///
/// Transforms old event versions to current versions during replay.
/// This is a passthrough upcaster - no transformations yet.
///
/// <h2>Adding Transformations</h2>
///
/// When schema evolution is needed, add transformations to the router:
/// <code>
/// private static UpcasterRouter CreateRouter()
/// {
///     return new UpcasterRouter("player")
///         .On("PlayerRegisteredV1", old => {
///             var v1 = old.Unpack&lt;PlayerRegisteredV1&gt;();
///             return Any.Pack(new PlayerRegistered {
///                 DisplayName = v1.DisplayName,
///                 Email = v1.Email,
///                 PlayerType = v1.PlayerType,
///                 AiModelId = ""  // New field with default
///             }, "type.googleapis.com/");
///         });
/// }
/// </code>
/// </summary>
public class Program
{
    public static void Main(string[] args)
    {
        var port = Environment.GetEnvironmentVariable("GRPC_PORT") ?? "50602";

        var builder = WebApplication.CreateBuilder(args);
        builder.Services.AddGrpc();
        builder.Services.AddSingleton(_ => CreateRouter());

        builder.WebHost.ConfigureKestrel(options =>
        {
            options.ListenAnyIP(
                int.Parse(port),
                o => o.Protocols = Microsoft.AspNetCore.Server.Kestrel.Core.HttpProtocols.Http2
            );
        });

        var app = builder.Build();
        app.MapGrpcService<PlayerUpcasterService>();

        Console.WriteLine($"Player upcaster listening on port {port}");
        app.Run();
    }

    // docs:start:upcaster_router
    /// <summary>
    /// Create the upcaster router for player domain.
    ///
    /// Currently a passthrough - add transformations as needed for schema evolution.
    /// </summary>
    private static UpcasterRouter CreateRouter()
    {
        return new UpcasterRouter("player");
        // Example transformation (uncomment when needed):
        // .On("PlayerRegisteredV1", old => {
        //     var v1 = old.Unpack<PlayerRegisteredV1>();
        //     return Any.Pack(new PlayerRegistered {
        //         DisplayName = v1.DisplayName,
        //         Email = v1.Email,
        //         PlayerType = v1.PlayerType,
        //         AiModelId = ""
        //     }, "type.googleapis.com/");
        // });
    }
    // docs:end:upcaster_router
}
