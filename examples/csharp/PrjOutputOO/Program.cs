using Microsoft.AspNetCore.Builder;
using Microsoft.AspNetCore.Hosting;
using Microsoft.Extensions.DependencyInjection;

namespace PrjOutputOO;

/// <summary>
/// Output projector (OO pattern) gRPC server.
///
/// Subscribes to player, table, and hand domain events.
/// Writes formatted game logs to a file.
///
/// This example demonstrates using the Projector base class with
/// [Projects(typeof(EventType))] annotated methods.
///
/// Compare with the functional pattern in PrjOutput.
/// </summary>
public class Program
{
    public static void Main(string[] args)
    {
        var port = Environment.GetEnvironmentVariable("GRPC_PORT") ?? "50692";
        var logFile = Environment.GetEnvironmentVariable("HAND_LOG_FILE") ?? "hand_log_oo.txt";

        // Clear log file at startup
        if (File.Exists(logFile))
        {
            File.Delete(logFile);
        }

        var builder = WebApplication.CreateBuilder(args);
        builder.Services.AddGrpc();
        builder.Services.AddSingleton<OutputProjectorService>();

        builder.WebHost.ConfigureKestrel(options =>
        {
            options.ListenAnyIP(
                int.Parse(port),
                o => o.Protocols = Microsoft.AspNetCore.Server.Kestrel.Core.HttpProtocols.Http2
            );
        });

        var app = builder.Build();
        app.MapGrpcService<OutputProjectorService>();

        Console.WriteLine($"Output projector (OO) listening on port {port}, logging to {logFile}");
        app.Run();
    }
}
