using Microsoft.AspNetCore.Builder;
using Microsoft.AspNetCore.Hosting;
using Microsoft.Extensions.DependencyInjection;

namespace HandFlowOO;

/// <summary>
/// Hand Flow process manager gRPC server entry point (OO pattern).
/// </summary>
public class Program
{
    public static void Main(string[] args)
    {
        var port = Environment.GetEnvironmentVariable("GRPC_PORT") ?? "50892";

        var builder = WebApplication.CreateBuilder(args);
        builder.Services.AddGrpc();

        builder.WebHost.ConfigureKestrel(options =>
        {
            options.ListenAnyIP(int.Parse(port), o => o.Protocols = Microsoft.AspNetCore.Server.Kestrel.Core.HttpProtocols.Http2);
        });

        var app = builder.Build();
        app.MapGrpcService<HandFlowService>();

        Console.WriteLine($"Hand Flow process manager (OO pattern) listening on port {port}");
        app.Run();
    }
}
