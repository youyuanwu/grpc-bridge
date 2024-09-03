using Helloworld;
using Grpc.Net.Client;

// disable server cert validation
var httpClientHandler = new HttpClientHandler
{
    ServerCertificateCustomValidationCallback = HttpClientHandler.DangerousAcceptAnyServerCertificateValidator
};
var httpClient = new HttpClient(httpClientHandler);

// TODO: switch between http and https
var channel = GrpcChannel.ForAddress("https://localhost:5047", new GrpcChannelOptions()
{
    HttpClient = httpClient
});

var client = new Greeter.GreeterClient(channel);

var response = await client.SayHelloAsync(
    new HelloRequest { Name = "World" });

Console.WriteLine(response.Message);
channel.Dispose();