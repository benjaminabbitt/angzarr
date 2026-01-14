using Angzarr.Examples.Projector;
using Serilog;
using Serilog.Formatting.Compact;

var builder = WebApplication.CreateBuilder(args);

Log.Logger = new LoggerConfiguration()
    .WriteTo.Console(new CompactJsonFormatter())
    .CreateLogger();

builder.Host.UseSerilog();

builder.Services.AddSingleton<Serilog.ILogger>(Log.Logger);
builder.Services.AddSingleton<IReceiptProjector, ReceiptProjector>();
builder.Services.AddGrpc();
builder.Services.AddGrpcHealthChecks();

var app = builder.Build();

app.MapGrpcService<ReceiptProjectorService>();
app.MapGrpcHealthChecksService();

var port = Environment.GetEnvironmentVariable("PORT") ?? "50055";
app.Urls.Add($"http://0.0.0.0:{port}");

Log.Information("Projector server started {@Data}",
    new { name = "receipt", port, listens_to = "transaction domain" });

try
{
    app.Run();
}
finally
{
    Log.CloseAndFlush();
}
