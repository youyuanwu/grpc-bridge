using Helloworld;
using Microsoft.AspNetCore.Builder;
using Microsoft.AspNetCore.Server.Kestrel.Core;
using Microsoft.Extensions.DependencyInjection;
using Microsoft.Extensions.Hosting;
using Microsoft.Extensions.Logging;
using Microsoft.VisualBasic;

namespace unit_tests;

public class GrpcServer
{

    public static (Task ShutdownTask, Task<Uri> UriTask) Run(CancellationToken cancellationToken)
    {
        var builder = WebApplication.CreateBuilder();

        // Configure to use port 0 to get an available port
        builder.Services.Configure<KestrelServerOptions>(options =>
        {
            options.ListenAnyIP(0, listenOptions =>
            {
                listenOptions.Protocols = HttpProtocols.Http2;
            });
        });

        // Add services to the container.
        builder.Services.AddGrpc();
        builder.Services.AddLogging(logging =>
        {
            logging.ClearProviders();
            logging.AddConsole();
            logging.SetMinimumLevel(LogLevel.Information);
        });

        var app = builder.Build();

        app.MapGrpcService<GreeterServerHello1>();

        // Register for cancellation and run the application
        cancellationToken.Register(app.Lifetime.StopApplication);

        // Start the application in the background
        var startTask = app.StartAsync();

        // Create a task to get the port once the server is ready
        var uriTask = Task.Run(async () =>
        {
            await startTask;

            var uri = app.Urls.Select(u => new Uri(u)).FirstOrDefault();
            if (uri != null && uri.Port != 0)
            {
                return uri;
            }

            throw new InvalidOperationException("Could not determine the listening port");
        });

        return (app.WaitForShutdownAsync(), uriTask);
    }
}

public class GreeterServerHello1(ILoggerFactory loggerFactory) : Greeter.GreeterBase
{
    public static readonly string ServiceName = nameof(GreeterServerHello1);

    private readonly ILogger<GreeterServerHello1> _logger = loggerFactory.CreateLogger<GreeterServerHello1>();

    public override Task<HelloReply> SayHello(HelloRequest request, Grpc.Core.ServerCallContext context)
    {
        _logger.LogInformation("Received request from {Name}", request.Name);
        return Task.FromResult(new HelloReply
        {
            Message = ServiceName + " " + request.Name
        });
    }
}


public class GreeterServerInProxy(ILoggerFactory loggerFactory) : Greeter.GreeterBase
{
    public static readonly string ServiceName = nameof(GreeterServerInProxy);

    private readonly ILogger<GreeterServerInProxy> _logger = loggerFactory.CreateLogger<GreeterServerInProxy>();

    public override Task<HelloReply> SayHello(HelloRequest request, Grpc.Core.ServerCallContext context)
    {
        _logger.LogInformation("Received request from {Name}", request.Name);
        return Task.FromResult(new HelloReply
        {
            Message = ServiceName + " " + request.Name
        });
    }
}