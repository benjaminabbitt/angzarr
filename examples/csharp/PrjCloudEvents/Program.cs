using Microsoft.AspNetCore.Builder;
using Microsoft.AspNetCore.Hosting;
using Microsoft.Extensions.DependencyInjection;

namespace PrjCloudEvents;

/// <summary>
/// CloudEvents projector gRPC server.
///
/// Transforms player domain events into CloudEvents format for external consumption.
/// Filters sensitive fields (email, internal IDs) before publishing.
/// </summary>
public class Program
{
    public static void Main(string[] args)
    {
        var port = Environment.GetEnvironmentVariable("PORT") ?? "50691";

        var builder = WebApplication.CreateBuilder(args);
        builder.Services.AddGrpc();
        builder.Services.AddSingleton<CloudEventsProjectorService>();

        builder.WebHost.ConfigureKestrel(options =>
        {
            options.ListenAnyIP(
                int.Parse(port),
                o => o.Protocols = Microsoft.AspNetCore.Server.Kestrel.Core.HttpProtocols.Http2
            );
        });

        var app = builder.Build();
        app.MapGrpcService<CloudEventsProjectorService>();

        Console.WriteLine($"CloudEvents projector listening on port {port}");
        app.Run();
    }
}
