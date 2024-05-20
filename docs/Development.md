# How to run

```ps1
.\build\examples\helloworld\Debug\greeter_server.exe
dotnet run --project .\src\greeter_server\greeter_server.csproj
dotnet run --project .\src\greeter_client\greeter_client.csproj
```

# Symcrypt
```ps1
# build
$env:SYMCRYPT_LIB_PATH="D:\code\cpp\grpc-bridge\build\_deps\symcrypt_release-src\dll"

# run time load
cp .\build\_deps\symcrypt_release-src\dll\symcrypt.dll .\target\debug\deps\   
```