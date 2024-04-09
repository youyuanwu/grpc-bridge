using System.Diagnostics;
using System.Net;
using System.Net.Sockets;
using Microsoft.AspNetCore.HttpLogging;
using Yarp.ReverseProxy.Forwarder;
using Yarp.ReverseProxy.Transforms;

var builder = WebApplication.CreateBuilder(args);

// Add services to the container.
// builder.Services.AddGrpc();
builder.Services.AddHttpLogging(logging =>
{
    logging.LoggingFields = HttpLoggingFields.All;
    logging.RequestBodyLogLimit = 4096;
    logging.ResponseBodyLogLimit = 4096;
    logging.CombineLogs = true;
});
builder.Services.AddHttpForwarder();

var app = builder.Build();

app.UseHttpLogging();

var logger = app.Logger;

string tempFolderPath = Path.GetTempPath();

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
    ConnectCallback = async (context, token) =>
    {
        logger.LogInformation("Connection httpClient.");
        var endpoint = new UnixDomainSocketEndPoint($"{tempFolderPath}/my.sock");
        var socket = new Socket(AddressFamily.Unix, SocketType.Stream, ProtocolType.Unspecified);

        try
        {
            await socket.ConnectAsync(endpoint, token).ConfigureAwait(false);
            return new NetworkStream(socket, true);
        }
        catch
        {
            socket.Dispose();
            throw;
        }
    }
});

// Setup our own request transform class
var transformer = new CustomTransformer(); // or HttpTransformer.Default;
var requestConfig = new ForwarderRequestConfig
{
    ActivityTimeout = TimeSpan.FromSeconds(100),
    Version = new Version("2.0"),
    VersionPolicy = HttpVersionPolicy.RequestVersionExact
};


app.UseRouting();

// For an alternate example that includes those features see BasicYarpSample.
app.Map("{*url}", async (string url, HttpContext httpContext, IHttpForwarder forwarder) =>
{
    logger.LogInformation("Got request url: {}", url);
    var error = await forwarder.SendAsync(httpContext, "http://localhost/",
        httpClient, requestConfig, transformer);
    // Check if the operation was successful
    if (error != ForwarderError.None)
    {
        var errorFeature = httpContext.GetForwarderErrorFeature() ?? throw new Exception("got error but no error feature");
        var exception = errorFeature.Exception;
        logger.LogInformation("Forward faile: {}", exception);
    }
});

app.Run();

/// <summary>
/// Custom request transformation
/// </summary>
internal class CustomTransformer : HttpTransformer
{
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

        // // Assign the custom uri. Be careful about extra slashes when concatenating here. RequestUtilities.MakeDestinationAddress is a safe default.
        proxyRequest.RequestUri = RequestUtilities.MakeDestinationAddress("http://localhost", httpContext.Request.Path, queryContext.QueryString);

        // Suppress the original request header, use the one from the destination Uri.
        proxyRequest.Headers.Host = null;
    }
}