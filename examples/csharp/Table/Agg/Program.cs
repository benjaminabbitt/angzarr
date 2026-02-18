using Microsoft.AspNetCore.Builder;
using Microsoft.AspNetCore.Hosting;
using Microsoft.Extensions.DependencyInjection;

namespace Table.Agg;

/// <summary>
/// Table aggregate gRPC server entry point.
/// </summary>
public class Program
{
    public static void Main(string[] args)
    {
        var port = Environment.GetEnvironmentVariable("GRPC_PORT") ?? "50602";

        var builder = WebApplication.CreateBuilder(args);
        builder.Services.AddGrpc();
        builder.Services.AddSingleton(_ => TableRouter.Create());
        builder.Services.AddSingleton<TableAggregate>();

        builder.WebHost.ConfigureKestrel(options =>
        {
            options.ListenAnyIP(int.Parse(port), o => o.Protocols = Microsoft.AspNetCore.Server.Kestrel.Core.HttpProtocols.Http2);
        });

        var app = builder.Build();
        app.MapGrpcService<TableAggregateService>();

        Console.WriteLine($"Table aggregate listening on port {port}");
        app.Run();
    }
}
