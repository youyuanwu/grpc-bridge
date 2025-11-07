using Helloworld;
using Microsoft.AspNetCore.Builder;
using Microsoft.AspNetCore.Server.Kestrel.Core;
using Microsoft.Extensions.DependencyInjection;
using Microsoft.Extensions.Hosting;
using Microsoft.Extensions.Logging;
using Microsoft.AspNetCore.Authorization;

namespace unit_tests;

public class GrpcServer
{

    public static (Task ShutdownTask, Task<Uri> UriTask) Run(CancellationToken cancellationToken, int instanceId)
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

        // Add authentication and authorization
        builder.Services.AddHeaderAuthentication(HeaderAuthConstants.TokenDirectPrefix);
        builder.Services.AddAuthorization();

        // Add GreeterMessagePrefix singleton
        builder.Services.AddSingleton(new GreeterMessagePrefix($"Hello{instanceId}"));

        builder.Services.AddLogging(logging =>
        {
            logging.ClearProviders();
            logging.AddConsole();
            logging.SetMinimumLevel(LogLevel.Information);
        });

        var app = builder.Build();

        app.UseRouting();

        // Add authentication and authorization middleware - MUST be between UseRouting() and endpoint mapping
        app.UseAuthentication();
        app.UseAuthorization();

        app.MapGrpcService<GreeterServer>();

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

public class GreeterMessagePrefix(string prefix)
{
    public readonly string Prefix = prefix;
}

[Authorize]
public class GreeterServer(ILoggerFactory loggerFactory, GreeterMessagePrefix messagePrefix) : Greeter.GreeterBase
{
    public readonly string ServiceName = messagePrefix.Prefix;

    private readonly ILogger<GreeterServer> _logger = loggerFactory.CreateLogger<GreeterServer>();

    public override Task<HelloReply> SayHello(HelloRequest request, Grpc.Core.ServerCallContext context)
    {
        _logger.LogInformation("{ServiceName} Received request from {Name}", ServiceName, request.Name);
        return Task.FromResult(new HelloReply
        {
            Message = ServiceName + " " + request.Name
        });
    }
}

