using Grpc.Core;
using Grpc.Core.Interceptors;
using Microsoft.Extensions.Logging;

namespace unit_tests;

public class ClientLoggingInterceptor : Interceptor
{
    private readonly ILoggerFactory _loggerFactory;
    private readonly ILogger<ClientLoggingInterceptor> _logger;

    public ClientLoggingInterceptor(ILoggerFactory loggerFactory)
    {
        _loggerFactory = loggerFactory;
        _logger = _loggerFactory.CreateLogger<ClientLoggingInterceptor>();
    }
    public override AsyncUnaryCall<TResponse> AsyncUnaryCall<TRequest, TResponse>(
        TRequest request,
        ClientInterceptorContext<TRequest, TResponse> context,
        AsyncUnaryCallContinuation<TRequest, TResponse> continuation)
        where TRequest : class
        where TResponse : class
    {

        _logger.LogInformation("Client sending request: {RequestType}", typeof(TRequest).Name);

        var call = continuation(request, context);

        // Wrap the response to add logging when the response is received
        var responseAsync = call.ResponseAsync.ContinueWith(task =>
        {
            if (task.IsCompletedSuccessfully)
            {
                _logger.LogInformation("Client received response: {ResponseType}", typeof(TResponse).Name);
            }
            else if (task.IsFaulted)
            {
                _logger.LogError(task.Exception, "Client request failed: {RequestType}", typeof(TRequest).Name);
            }
            return task.Result;
        });

        return new AsyncUnaryCall<TResponse>(
            responseAsync,
            call.ResponseHeadersAsync,
            call.GetStatus,
            call.GetTrailers,
            call.Dispose);
    }
}

public static class GrpcClientUtil
{

    public static Uri ReplaceToLocalhost(Uri uri)
    {
        var builder = new UriBuilder(uri)
        {
            Host = "localhost"
        };
        return builder.Uri;
    }
}

/// <summary>
/// A delegating handler that prepends a path prefix to all HTTP requests.
/// </summary>
public class HttpPathPrefixHandler : DelegatingHandler
{
    private readonly string _pathPrefix;

    public HttpPathPrefixHandler(string pathPrefix, HttpMessageHandler? innerHandler = null)
    {
        _pathPrefix = NormalizePath(pathPrefix);
        InnerHandler = innerHandler ?? new HttpClientHandler();
    }

    protected override async Task<HttpResponseMessage> SendAsync(HttpRequestMessage request, CancellationToken cancellationToken)
    {
        if (request.RequestUri != null)
        {
            var originalPath = request.RequestUri.AbsolutePath;
            var prefixedPath = CombinePaths(_pathPrefix, originalPath);

            var builder = new UriBuilder(request.RequestUri)
            {
                Path = prefixedPath
            };

            request.RequestUri = builder.Uri;
        }

        return await base.SendAsync(request, cancellationToken);
    }

    private static string NormalizePath(string path)
    {
        if (string.IsNullOrEmpty(path))
            return string.Empty;

        // Ensure path starts with / but doesn't end with /
        if (!path.StartsWith("/"))
            path = "/" + path;

        if (path.Length > 1 && path.EndsWith("/"))
            path = path.TrimEnd('/');

        return path;
    }

    private static string CombinePaths(string prefix, string path)
    {
        if (string.IsNullOrEmpty(prefix))
            return path;

        if (string.IsNullOrEmpty(path) || path == "/")
            return prefix;

        // Remove leading slash from path if present to avoid double slashes
        if (path.StartsWith("/"))
            path = path.Substring(1);

        return prefix + "/" + path;
    }
}