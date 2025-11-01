using Grpc.Core.Interceptors;
using Grpc.Core;
using Grpc.Net.Client;
using Helloworld;
using Microsoft.Extensions.Logging;

namespace unit_tests;

public class TestConstants
{
    public const string PathProxyPrefix = "/ProxyGrpc";
}


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

        var (grpcShutdownTask, grpcUriTask) = GrpcServer.Run(cts.Token);
        var localTargetUri = GrpcClientUtil.ReplaceToLocalhost(await grpcUriTask);
        var (shutdownTask, uriTask) = ProxyServer.RunAsync(localTargetUri, TestConstants.PathProxyPrefix, loggerFactory, cts.Token);
        var localProxyUri = GrpcClientUtil.ReplaceToLocalhost(await uriTask);
        // wait ready.
        var uri = await uriTask;
        logger.LogInformation($"Proxy is listening on {localProxyUri}");
        logger.LogInformation($"gRPC server is listening on {localTargetUri}");

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

            var reply = await client.SayHelloAsync(new HelloRequest { Name = "World" });
            logger.LogInformation($"Received reply: {reply.Message}");
            Assert.AreEqual(GreeterServerHello1.ServiceName + " World", reply.Message);
        }

        // Send a request to proxy that should not be forwarded
        {
            var client = new Greeter.GreeterClient(GrpcChannel.ForAddress(localProxyUri, new GrpcChannelOptions
            {
                LoggerFactory = loggerFactory
            }));
            var reply = await client.SayHelloAsync(new HelloRequest { Name = "Direct" });
            logger.LogInformation($"Received reply: {reply.Message}");
            Assert.AreEqual(GreeterServerInProxy.ServiceName + " Direct", reply.Message);
        }

        // Cancel the proxy server
        cts.Cancel();

        await shutdownTask;
        await grpcShutdownTask;
    }
}