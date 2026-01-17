using Angzarr.Examples.SagaFulfillment;
using Serilog;
using Serilog.Formatting.Compact;

var builder = WebApplication.CreateBuilder(args);

Log.Logger = new LoggerConfiguration()
    .WriteTo.Console(new CompactJsonFormatter())
    .CreateLogger();

builder.Host.UseSerilog();

builder.Services.AddSingleton<Serilog.ILogger>(Log.Logger);
builder.Services.AddGrpc();
builder.Services.AddGrpcHealthChecks();

var app = builder.Build();

app.MapGrpcService<FulfillmentSagaService>();
app.MapGrpcHealthChecksService();

var port = Environment.GetEnvironmentVariable("PORT") ?? "50607";
app.Urls.Add($"http://0.0.0.0:{port}");

Log.Information("Saga server started {@Data}",
    new { saga = "fulfillment", port, source_domain = "order" });

try
{
    app.Run();
}
finally
{
    Log.CloseAndFlush();
}
