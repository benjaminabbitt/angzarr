using Microsoft.AspNetCore.Builder;
using Microsoft.AspNetCore.Hosting;
using Microsoft.Extensions.DependencyInjection;

namespace Table.SagaHandOO;

/// <summary>
/// Spring Boot application for Table -> Hand saga using OO pattern.
///
/// This example demonstrates using the Saga base class with
/// annotation-based handler registration ([Prepares], [Handles]).
///
/// Compare with the functional EventRouter pattern in Table/SagaHand.
/// </summary>
public class Program
{
    public static void Main(string[] args)
    {
        var port = Environment.GetEnvironmentVariable("GRPC_PORT") ?? "50640";

        var builder = WebApplication.CreateBuilder(args);
        builder.Services.AddGrpc();
        builder.Services.AddSingleton<TableHandSagaService>();

        builder.WebHost.ConfigureKestrel(options =>
        {
            options.ListenAnyIP(
                int.Parse(port),
                o => o.Protocols = Microsoft.AspNetCore.Server.Kestrel.Core.HttpProtocols.Http2
            );
        });

        var app = builder.Build();
        app.MapGrpcService<TableHandSagaService>();

        Console.WriteLine($"Table->Hand saga (OO) listening on port {port}");
        app.Run();
    }
}
