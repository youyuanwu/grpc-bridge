using System.Diagnostics;
using System.Net;
using System.Net.Sockets;
using Microsoft.AspNetCore.Builder;
using Microsoft.AspNetCore.Hosting;
using Microsoft.AspNetCore.Http;
using Microsoft.AspNetCore.HttpLogging;
using Microsoft.AspNetCore.Server.Kestrel.Core;
using Microsoft.Extensions.DependencyInjection;
using Microsoft.Extensions.Hosting;
using Microsoft.Extensions.Logging;
using Yarp.ReverseProxy.Forwarder;
using Yarp.ReverseProxy.Transforms;

namespace unit_tests;

public class ProxyServer
{
    public static (Task ShutdownTask, Task<Uri> UriTask) RunAsync(Uri targetUri, string proxyPrefix, ILoggerFactory loggerFactory, CancellationToken cancellationToken)
    {
        var logger = loggerFactory.CreateLogger<ProxyServer>();

        var builder = WebApplication.CreateBuilder();

        // Clear the default logging providers and add the provided logger factory
        builder.Logging.ClearProviders();
        builder.Services.AddSingleton<ILoggerFactory>(loggerFactory);

        // Configure to use port 0 to get an available port and enable HTTP/2 cleartext (H2C)
        builder.Services.Configure<KestrelServerOptions>(options =>
        {
            options.ListenAnyIP(0, listenOptions =>
            {
                listenOptions.Protocols = HttpProtocols.Http2;
            });
        });

        // Add services to the container.
        // builder.Services.AddGrpc();
        builder.Services.AddHttpLogging(logging =>
        {
            logging.LoggingFields = HttpLoggingFields.All;
            logging.RequestBodyLogLimit = 4096;
            logging.ResponseBodyLogLimit = 4096;
            logging.CombineLogs = true;
        });
        builder.Services.AddGrpc();
        builder.Services.AddHttpForwarder();

        var app = builder.Build();

        // Use HTTP logging only for non-gRPC endpoints
        app.UseWhen(context => !context.Request.ContentType?.StartsWith("application/grpc") == true,
            appBuilder => appBuilder.UseHttpLogging());

        // Proxy itself runs a grpc server.
        app.MapGrpcService<GreeterServerInProxy>();

        // Configure our own HttpMessageInvoker for outbound calls for proxy operations
        var httpClient = new HttpMessageInvoker(new SocketsHttpHandler
        {
            UseProxy = false,
            AllowAutoRedirect = false,
            AutomaticDecompression = DecompressionMethods.None,
            UseCookies = false,
            EnableMultipleHttp2Connections = true,
            ActivityHeadersPropagator = new ReverseProxyPropagator(DistributedContextPropagator.Current),
            ConnectTimeout = TimeSpan.FromSeconds(15),
        });

        // Setup our own request transform class
        var transformer = new CustomTransformer(proxyPrefix, loggerFactory); // or HttpTransformer.Default;
        var requestConfig = new ForwarderRequestConfig
        {
            ActivityTimeout = TimeSpan.FromSeconds(100),
            Version = new Version("2.0"),
            VersionPolicy = HttpVersionPolicy.RequestVersionExact
        };

        app.UseRouting();

        // Add a catch-all middleware to debug routing
        app.Use(async (context, next) =>
        {
            logger.LogInformation("Middleware: Request {method} {path}", context.Request.Method, context.Request.Path);
            await next();
        });

        // For an alternate example that includes those features see BasicYarpSample.
        app.MapPost(proxyPrefix + "/{**catch-all}", async (HttpContext httpContext, IHttpForwarder forwarder, ILogger<ProxyServer> injectedLogger) =>
        {
            var error = await forwarder.SendAsync(httpContext, targetUri.ToString(),
                httpClient, requestConfig, transformer);
            // Check if the operation was successful
            if (error != ForwarderError.None)
            {
                var errorFeature = httpContext.GetForwarderErrorFeature() ?? throw new Exception("got error but no error feature");
                var exception = errorFeature.Exception;
                injectedLogger.LogError("Injected logger: Forward failed: {exception}", exception);
            }

        });

        // Add a fallback route for debugging
        app.MapFallback(async (HttpContext httpContext, ILogger<ProxyServer> injectedLogger) =>
        {
            injectedLogger.LogInformation("Fallback: Request {method} {path}", httpContext.Request.Method, httpContext.Request.Path);
            httpContext.Response.StatusCode = 404;
            await httpContext.Response.WriteAsync("No matching route found");
        });

        // Register the cancellation token to stop the application
        cancellationToken.Register(app.Lifetime.StopApplication);

        // Create a task completion source for the port
        var portTaskCompletionSource = new TaskCompletionSource<int>();

        // Start the application in the background
        var startTask = app.StartAsync();

        logger.LogInformation("Proxy server started.");

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



/// <summary>
/// Custom request transformation
/// </summary>
internal class CustomTransformer(string proxyPrefix, ILoggerFactory loggerFactory) : HttpTransformer
{
    private readonly string _proxyPrefix = proxyPrefix;
    private readonly ILogger _logger = loggerFactory.CreateLogger<CustomTransformer>();
    /// <summary>
    /// A callback that is invoked prior to sending the proxied request. All HttpRequestMessage
    /// fields are initialized except RequestUri, which will be initialized after the
    /// callback if no value is provided. The string parameter represents the destination
    /// URI prefix that should be used when constructing the RequestUri. The headers
    /// are copied by the base implementation, excluding some protocol headers like HTTP/2
    /// pseudo headers (":authority").
    /// </summary>
    /// <param name="httpContext">The incoming request.</param>
    /// <param name="proxyRequest">The outgoing proxy request.</param>
    /// <param name="destinationPrefix">The uri prefix for the selected destination server which can be used to create
    /// the RequestUri.</param>
    public override async ValueTask TransformRequestAsync(HttpContext httpContext, HttpRequestMessage proxyRequest, string destinationPrefix, CancellationToken cancellationToken)
    {
        // Copy all request headers
        await base.TransformRequestAsync(httpContext, proxyRequest, destinationPrefix, cancellationToken);

        // // Customize the query string:
        var queryContext = new QueryTransformContext(httpContext.Request);
        // queryContext.Collection.Remove("param1");
        // queryContext.Collection["area"] = "xx2";

        // remove the proxy prefix from the path
        var originalPath = httpContext.Request.Path.ToString();
        if (originalPath.StartsWith(_proxyPrefix))
        {
            originalPath = originalPath.Substring(_proxyPrefix.Length);
        }

        // // Assign the custom uri. Be careful about extra slashes when concatenating here. RequestUtilities.MakeDestinationAddress is a safe default.
        // Use the provided destinationPrefix instead of hardcoding localhost
        proxyRequest.RequestUri = RequestUtilities.MakeDestinationAddress(destinationPrefix, originalPath, queryContext.QueryString);
        _logger.LogInformation("Transformed request URI: {}", proxyRequest.RequestUri);
        // Suppress the original request header, use the one from the destination Uri.
        proxyRequest.Headers.Host = null;
    }
}