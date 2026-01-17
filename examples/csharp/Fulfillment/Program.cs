using Angzarr.Examples.Fulfillment;
using Serilog;
using Serilog.Formatting.Compact;

var builder = WebApplication.CreateBuilder(args);

Log.Logger = new LoggerConfiguration()
    .WriteTo.Console(new CompactJsonFormatter())
    .CreateLogger();

builder.Host.UseSerilog();

builder.Services.AddSingleton<Serilog.ILogger>(Log.Logger);
builder.Services.AddSingleton<IFulfillmentLogic, FulfillmentLogic>();
builder.Services.AddGrpc();
builder.Services.AddGrpcHealthChecks();

var app = builder.Build();

app.MapGrpcService<FulfillmentService>();
app.MapGrpcHealthChecksService();

var port = Environment.GetEnvironmentVariable("PORT") ?? "50605";
app.Urls.Add($"http://0.0.0.0:{port}");

Log.Information("Business logic server started {@Data}",
    new { domain = "fulfillment", port });

try
{
    app.Run();
}
finally
{
    Log.CloseAndFlush();
}
