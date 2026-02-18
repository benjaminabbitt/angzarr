using Microsoft.AspNetCore.Builder;
using Microsoft.AspNetCore.Hosting;
using Microsoft.Extensions.DependencyInjection;

namespace Hand.Agg;

/// <summary>
/// Hand aggregate gRPC server entry point.
/// </summary>
public class Program
{
    public static void Main(string[] args)
    {
        var port = Environment.GetEnvironmentVariable("GRPC_PORT") ?? "50603";

        var builder = WebApplication.CreateBuilder(args);
        builder.Services.AddGrpc();
        builder.Services.AddSingleton(_ => HandRouter.Create());
        builder.Services.AddSingleton<HandAggregate>();

        builder.WebHost.ConfigureKestrel(options =>
        {
            options.ListenAnyIP(int.Parse(port), o => o.Protocols = Microsoft.AspNetCore.Server.Kestrel.Core.HttpProtocols.Http2);
        });

        var app = builder.Build();
        app.MapGrpcService<HandAggregateService>();

        Console.WriteLine($"Hand aggregate listening on port {port}");
        app.Run();
    }
}
