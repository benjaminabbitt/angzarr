using Angzarr.Examples.Saga;
using Serilog;
using Serilog.Formatting.Compact;

var builder = WebApplication.CreateBuilder(args);

Log.Logger = new LoggerConfiguration()
    .WriteTo.Console(new CompactJsonFormatter())
    .CreateLogger();

builder.Host.UseSerilog();

builder.Services.AddSingleton<Serilog.ILogger>(Log.Logger);
builder.Services.AddSingleton<ILoyaltySaga, LoyaltySaga>();
builder.Services.AddGrpc();
builder.Services.AddGrpcHealthChecks();

var app = builder.Build();

app.MapGrpcService<LoyaltySagaService>();
app.MapGrpcHealthChecksService();

var port = Environment.GetEnvironmentVariable("PORT") ?? "50054";
app.Urls.Add($"http://0.0.0.0:{port}");

Log.Information("Saga server started {@Data}",
    new { name = "loyalty_points", port, listens_to = "transaction domain" });

try
{
    app.Run();
}
finally
{
    Log.CloseAndFlush();
}
