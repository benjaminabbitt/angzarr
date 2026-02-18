using Microsoft.AspNetCore.Builder;
using Microsoft.AspNetCore.Hosting;
using Microsoft.Extensions.DependencyInjection;

namespace PrjOutput;

/// <summary>
/// Output projector gRPC server entry point.
/// </summary>
public class Program
{
    public static void Main(string[] args)
    {
        var port = Environment.GetEnvironmentVariable("GRPC_PORT") ?? "50690";
        var logFile = Environment.GetEnvironmentVariable("HAND_LOG_FILE") ?? "hand_log.txt";

        // Clear log file at startup
        if (File.Exists(logFile))
        {
            File.Delete(logFile);
        }

        var builder = WebApplication.CreateBuilder(args);
        builder.Services.AddGrpc();
        builder.Services.AddSingleton(_ => new OutputProjectorService(logFile));

        builder.WebHost.ConfigureKestrel(options =>
        {
            options.ListenAnyIP(int.Parse(port), o => o.Protocols = Microsoft.AspNetCore.Server.Kestrel.Core.HttpProtocols.Http2);
        });

        var app = builder.Build();
        app.MapGrpcService<OutputProjectorService>();

        Console.WriteLine($"Output projector listening on port {port}, logging to {logFile}");
        app.Run();
    }
}
