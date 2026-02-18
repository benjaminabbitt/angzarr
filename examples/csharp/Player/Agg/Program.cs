using Microsoft.AspNetCore.Builder;
using Microsoft.AspNetCore.Hosting;
using Microsoft.Extensions.DependencyInjection;

namespace Player.Agg;

/// <summary>
/// Player aggregate gRPC server entry point.
/// </summary>
public class Program
{
    public static void Main(string[] args)
    {
        var port = Environment.GetEnvironmentVariable("GRPC_PORT") ?? "50601";

        var builder = WebApplication.CreateBuilder(args);
        builder.Services.AddGrpc();
        builder.Services.AddSingleton(_ => PlayerRouter.Create());
        builder.Services.AddSingleton<PlayerAggregate>();

        builder.WebHost.ConfigureKestrel(options =>
        {
            options.ListenAnyIP(int.Parse(port), o => o.Protocols = Microsoft.AspNetCore.Server.Kestrel.Core.HttpProtocols.Http2);
        });

        var app = builder.Build();
        app.MapGrpcService<PlayerAggregateService>();

        Console.WriteLine($"Player aggregate listening on port {port}");
        app.Run();
    }
}
