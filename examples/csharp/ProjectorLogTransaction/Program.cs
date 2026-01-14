using Angzarr.Examples.Projector;
using Serilog;
using Serilog.Formatting.Compact;

var builder = WebApplication.CreateBuilder(args);

Log.Logger = new LoggerConfiguration()
    .WriteTo.Console(new CompactJsonFormatter())
    .CreateLogger();

builder.Host.UseSerilog();

builder.Services.AddSingleton<Serilog.ILogger>(Log.Logger);
builder.Services.AddSingleton<ITransactionLogProjector, TransactionLogProjector>();
builder.Services.AddGrpc();
builder.Services.AddGrpcHealthChecks();

var app = builder.Build();

app.MapGrpcService<TransactionLogProjectorService>();
app.MapGrpcHealthChecksService();

var port = Environment.GetEnvironmentVariable("PORT") ?? "50057";
app.Urls.Add($"http://0.0.0.0:{port}");

Log.Information("Projector server started {@Data}",
    new { name = "log-transaction", port, listens_to = "transaction domain" });

try
{
    app.Run();
}
finally
{
    Log.CloseAndFlush();
}
