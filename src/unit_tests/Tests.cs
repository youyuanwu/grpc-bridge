using Grpc.Core;
using Grpc.Core.Interceptors;
using Grpc.Net.Client;
using Helloworld;
using Microsoft.Extensions.Logging;

namespace unit_tests;

public class TestConstants
{
    public const string PathProxyPrefix = "/ProxyGrpc";
}

/// <summary>
/// Tests for the gRPC proxy server
/// Grpc servers instances are created and a proxy server is started to route requests
/// to the appropriate instance based on headers.
/// Proxy also replaces the authentication headers.
/// Proxy itself also responds to direct gRPC calls.
/// </summary>
[TestClass]
public class ProxyTest
{
    [TestMethod]
    public async Task TestProxy()
    {
        // Create a logger factory for testing
        using var loggerFactory = LoggerFactory.Create(builder =>
        {
            builder
                .AddConsole()
                .SetMinimumLevel(LogLevel.Information);
        });
        var logger = loggerFactory.CreateLogger<ProxyTest>();
        using var cts = new CancellationTokenSource();

        var (grpcShutdownTask1, grpcUriTask1) = GrpcServer.Run(cts.Token, 1);
        var (grpcShutdownTask2, grpcUriTask2) = GrpcServer.Run(cts.Token, 2);
        var localTargetUri1 = GrpcClientUtil.ReplaceToLocalhost(await grpcUriTask1);
        var localTargetUri2 = GrpcClientUtil.ReplaceToLocalhost(await grpcUriTask2);
        // Map instance IDs to their local target URIs
        var addrMap = new Dictionary<int, Uri>
        {
            { 1, localTargetUri1 },
            { 2, localTargetUri2 }
        };
        var (shutdownTask, uriTask) = ProxyServer.RunAsync(addrMap, TestConstants.PathProxyPrefix, loggerFactory, cts.Token);
        var localProxyUri = GrpcClientUtil.ReplaceToLocalhost(await uriTask);
        // wait ready.
        var uri = await uriTask;
        logger.LogInformation($"Proxy is listening on {localProxyUri}");
        logger.LogInformation($"gRPC server is listening on {localTargetUri1}");

        // Send a test grpc request to the proxy and it should be routed
        {
            var grpcOpt = new GrpcChannelOptions
            {
                LoggerFactory = loggerFactory,
                HttpHandler = new HttpPathPrefixHandler(TestConstants.PathProxyPrefix)
            };
            var interceptor = new ClientLoggingInterceptor(loggerFactory);
            var channel = GrpcChannel.ForAddress(localProxyUri, grpcOpt);
            var callInvoker = channel.CreateCallInvoker().Intercept(interceptor);
            var client = new Greeter.GreeterClient(callInvoker);

            // Send request to be routed to instance 1
            {
                var headers = new Metadata
                {
                    // Add authorization header for the proxy call
                    { HeaderAuthConstants.AuthorizationHeader, HeaderAuthConstants.TokenProxyPrefix + "test-token-123" },
                    // Add target instance ID header to route to instance 1
                    { GrpcProxyConstants.TargetInstanceIdHeader, "1" }
                };

                var reply = await client.SayHelloAsync(new HelloRequest { Name = "World" }, headers: headers);
                logger.LogInformation($"Received reply: {reply.Message}");
                Assert.AreEqual("Hello1" + " World", reply.Message);
            }
            // Send request to be routed to instance 2
            {
                var headers = new Metadata
                {
                    // Add authorization header for the proxy call
                    { HeaderAuthConstants.AuthorizationHeader,  HeaderAuthConstants.TokenProxyPrefix + "test-token-123" },
                    // Add target instance ID header to route to instance 2
                    { GrpcProxyConstants.TargetInstanceIdHeader, "2" }
                };

                var reply = await client.SayHelloAsync(new HelloRequest { Name = "Everyone" }, headers: headers);
                logger.LogInformation($"Received reply: {reply.Message}");
                Assert.AreEqual("Hello2" + " Everyone", reply.Message);
            }
            // Send request to unknown instance ID
            {
                var headers = new Metadata
                {
                    // Add authorization header for the proxy call
                    { HeaderAuthConstants.AuthorizationHeader,  HeaderAuthConstants.TokenProxyPrefix + "test-token-123" },
                    // Add target instance ID header to route to instance 99 (unknown)
                    { GrpcProxyConstants.TargetInstanceIdHeader, "99" }
                };

                try
                {
                    var reply = await client.SayHelloAsync(new HelloRequest { Name = "Unknown" }, headers: headers);
                    Assert.Fail("Expected RpcException was not thrown");
                }
                catch (RpcException ex)
                {
                    logger.LogInformation($"Received expected RpcException: {ex.Status.Detail}");
                    Assert.AreEqual(StatusCode.Unimplemented, ex.StatusCode);
                }
            }
        }

        // Send a request to proxy that should not be forwarded
        {
            var client = new Greeter.GreeterClient(GrpcChannel.ForAddress(localProxyUri, new GrpcChannelOptions
            {
                LoggerFactory = loggerFactory
            }));

            // Add authorization header for the direct proxy gRPC call
            // Proxy prefix is used for the proxy server direct.
            var headers = new Metadata
            {
                { HeaderAuthConstants.AuthorizationHeader, HeaderAuthConstants.TokenProxyPrefix + "test-token-456" }
            };

            var reply = await client.SayHelloAsync(new HelloRequest { Name = "Direct" }, headers: headers);
            logger.LogInformation($"Received reply: {reply.Message}");
            Assert.AreEqual("Proxy" + " Direct", reply.Message);
        }

        // Cancel the proxy server
        cts.Cancel();

        await shutdownTask;
        await grpcShutdownTask1;
    }
}