# grpc-bridge
Various examples of using grpc.


## grpc-dotnet proxy for grpc cpp via unix socket.
This is especially useful for proxying grpc cpp on windows. grpc cpp (core) does not support windows cert store, but grpc-dotnet/asp.net does. So using grpc-dotnet with tls as a proxy is a natual choice.

The example has grpc cpp app listens on a unix socket, and asp.net kestrel server routes all http request to the unix socket.

### requirements
* grpc cpp
* dotnet 
### build
```
cmake . -B build
cmake --build build 
```

### gRPC Rust codegen tools

Install the pinned `protoc` and `protoc-gen-rust-grpc` binaries locally:

```
cmake -P cmake/install-grpc-rust-tools.cmake
```

The tools are installed in `.tools/grpc-rust/bin`. The `grpc-tests` build
script uses this fixed repository-local path, so no `PATH` changes are needed.

### Run example
```ps1
# start the server on unix socket
.\build\examples\helloworld\Debug\greeter_server.exe
# start asp.net proxy
dotnet run --project .\src\greeter_server\greeter_server.csproj
# make a request to the proxy
dotnet run --project .\src\greeter_client\greeter_client.csproj
```