using Angzarr.Examples.Cart;
using Serilog;
using Serilog.Formatting.Compact;

var builder = WebApplication.CreateBuilder(args);

Log.Logger = new LoggerConfiguration()
    .WriteTo.Console(new CompactJsonFormatter())
    .CreateLogger();

builder.Host.UseSerilog();

builder.Services.AddSingleton<Serilog.ILogger>(Log.Logger);
builder.Services.AddSingleton<ICartLogic, CartLogic>();
builder.Services.AddGrpc();
builder.Services.AddGrpcHealthChecks();

var app = builder.Build();

app.MapGrpcService<CartService>();
app.MapGrpcHealthChecksService();

var port = Environment.GetEnvironmentVariable("PORT") ?? "50602";
app.Urls.Add($"http://0.0.0.0:{port}");

Log.Information("Business logic server started {@Data}",
    new { domain = "cart", port });

try
{
    app.Run();
}
finally
{
    Log.CloseAndFlush();
}
