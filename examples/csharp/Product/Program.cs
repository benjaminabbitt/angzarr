using Angzarr.Examples.Product;
using Serilog;
using Serilog.Formatting.Compact;

var builder = WebApplication.CreateBuilder(args);

Log.Logger = new LoggerConfiguration()
    .WriteTo.Console(new CompactJsonFormatter())
    .CreateLogger();

builder.Host.UseSerilog();

builder.Services.AddSingleton<Serilog.ILogger>(Log.Logger);
builder.Services.AddSingleton<IProductLogic, ProductLogic>();
builder.Services.AddGrpc();
builder.Services.AddGrpcHealthChecks();

var app = builder.Build();

app.MapGrpcService<ProductService>();
app.MapGrpcHealthChecksService();

var port = Environment.GetEnvironmentVariable("PORT") ?? "50601";
app.Urls.Add($"http://0.0.0.0:{port}");

Log.Information("Business logic server started {@Data}",
    new { domain = "product", port });

try
{
    app.Run();
}
finally
{
    Log.CloseAndFlush();
}
