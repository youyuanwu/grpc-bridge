use crate::generated::{OwnedHelloReply, OwnedHelloRequest};

pub mod generated;

pub struct Greeter {}

#[tonic::async_trait]
impl generated::greeter_server::Greeter for Greeter {
    async fn say_hello(
        &self,
        request: tonic::Request<OwnedHelloRequest>,
    ) -> Result<tonic::Response<OwnedHelloReply>, tonic::Status> {
        let request = request.into_inner();
        let name = request.get_ref().name();
        println!("Got a name: {name:?}");
        let mut builder = flatbuffers_util::FBBuilder::new();
        let hello_str = builder
            .get_mut()
            .create_string(&format!("hello {}", name.unwrap_or("")));
        let reply = generated::greeter::HelloReply::create(
            builder.get_mut(),
            &generated::greeter::HelloReplyArgs {
                message: Some(hello_str),
            },
        );
        let resp = builder.finish_owned(reply).into();
        Ok(tonic::Response::new(resp))
    }

    type SayManyHellosStream =
        tokio_stream::wrappers::ReceiverStream<Result<generated::OwnedHelloReply, tonic::Status>>;
    async fn say_many_hellos(
        &self,
        request: tonic::Request<generated::OwnedManyHellosRequest>,
    ) -> Result<tonic::Response<Self::SayManyHellosStream>, tonic::Status> {
        let (tx, rx) = tokio::sync::mpsc::channel(4);
        tokio::spawn(async move {
            let request = request.into_inner();
            let name = request.get_ref().name();
            let num_greetings = request.get_ref().num_greetings();
            println!("Got name: {name:?}");
            for _ in 0..num_greetings {
                let mut builder = flatbuffers_util::FBBuilder::new();
                let hello_str = builder
                    .get_mut()
                    .create_string(&format!("hello {}", name.unwrap_or("")));
                let reply = generated::greeter::HelloReply::create(
                    builder.get_mut(),
                    &generated::greeter::HelloReplyArgs {
                        message: Some(hello_str),
                    },
                );
                let resp = builder.finish_owned(reply).into();
                let owned_reply: generated::OwnedHelloReply = resp;
                if let Err(e) = tx.send(Ok(owned_reply)).await {
                    eprintln!("Failed to send reply: {}", e);
                    return;
                }
            }
        });
        Ok(tonic::Response::new(
            tokio_stream::wrappers::ReceiverStream::new(rx),
        ))
    }
}

#[cfg(test)]
mod tests {
    use tokio_util::sync::CancellationToken;

    use crate::generated::OwnedHelloRequest;

    // semaphore to limit concurrent test runs
    static SEM: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(1);

    // creates a listener on a random port from os, and return the addr.
    pub async fn create_listener_server() -> (tokio::net::TcpListener, std::net::SocketAddr) {
        let addr: std::net::SocketAddr = "127.0.0.1:50051".parse().unwrap();
        let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
        let local_addr = listener.local_addr().unwrap();
        (listener, local_addr)
    }

    fn get_root_dir() -> std::path::PathBuf {
        let manifest_dir = std::env!("CARGO_MANIFEST_DIR");
        let path = std::path::Path::new(manifest_dir).parent().unwrap();
        path.to_path_buf()
    }

    fn get_cpp_client_exe() -> std::path::PathBuf {
        let mut path = get_root_dir();
        path.push("build");
        path.push("flatbuffers-test");
        path.push("cpp");
        if cfg!(target_os = "windows") {
            path.push("Debug");
        }
        path.push("fbs_greeter_client");
        if cfg!(target_os = "windows") {
            path.set_extension("exe");
        }
        // assert exists
        assert!(path.exists(), "cpp client exe not found: {:?}", path);
        path
    }

    fn get_cpp_server_exe() -> std::path::PathBuf {
        let mut path = get_root_dir();
        path.push("build");
        path.push("flatbuffers-test");
        path.push("cpp");
        if cfg!(target_os = "windows") {
            path.push("Debug");
        }
        path.push("fbs_greeter_server");
        if cfg!(target_os = "windows") {
            path.set_extension("exe");
        }
        // assert exists
        assert!(path.exists(), "cpp server exe not found: {:?}", path);
        path
    }

    #[tokio::test]
    async fn tonic_server_cpp_client() {
        let _permit = SEM.acquire().await.unwrap();
        let (listener, addr) = create_listener_server().await;
        let token = CancellationToken::new();

        // run server in task
        let svh = {
            let token = token.clone();
            tokio::spawn(async move {
                let svc = crate::generated::greeter_server::GreeterServer::new(crate::Greeter {});
                tonic::transport::Server::builder()
                    .add_service(svc)
                    .serve_with_incoming_shutdown(
                        tonic::transport::server::TcpIncoming::from(listener),
                        token.cancelled(),
                    )
                    .await
                    .unwrap();
            })
        };

        // run client
        let mut client =
            crate::generated::greeter_client::GreeterClient::connect(format!("http://{}", addr))
                .await
                .unwrap();
        let mut builder = flatbuffers_util::FBBuilder::new();
        let name = builder.get_mut().create_string("world1");
        let req = crate::generated::greeter::HelloRequest::create(
            builder.get_mut(),
            &crate::generated::greeter::HelloRequestArgs { name: Some(name) },
        );
        let owned_req = builder.finish_owned(req);
        let response = client
            .say_hello(tonic::Request::new(OwnedHelloRequest::from(owned_req)))
            .await
            .unwrap();
        let reply = response.into_inner();
        let message = reply.get_ref().message();
        assert_eq!(message.unwrap(), "hello world1");

        // run cpp client exe using tokio::process and share stdout/stderr
        let cpp_client_exe = get_cpp_client_exe();
        let status = tokio::process::Command::new(cpp_client_exe)
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .status()
            .await
            .unwrap();
        assert!(status.success());

        // shutdown server
        token.cancel();
        svh.await.unwrap();
    }

    #[tokio::test]
    async fn tonic_client_cpp_server() {
        let _permit = SEM.acquire().await.unwrap();
        let token = CancellationToken::new();
        let addr = "localhost:50051";
        // run server exe in task
        let svh = {
            let token = token.clone();
            tokio::spawn(async move {
                let cpp_server_exe = get_cpp_server_exe();
                let mut child = tokio::process::Command::new(cpp_server_exe)
                    .stdout(std::process::Stdio::inherit())
                    .stderr(std::process::Stdio::inherit())
                    .spawn()
                    .unwrap();
                // wait for cancellation
                token.cancelled().await;
                // kill the process
                child.kill().await.unwrap();
            })
        };

        // give server some time to start
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        // run client
        let mut client =
            crate::generated::greeter_client::GreeterClient::connect(format!("http://{}", addr))
                .await
                .unwrap();
        let mut builder = flatbuffers_util::FBBuilder::new();
        let name = builder.get_mut().create_string("world3");
        let req = crate::generated::greeter::HelloRequest::create(
            builder.get_mut(),
            &crate::generated::greeter::HelloRequestArgs { name: Some(name) },
        );
        let owned_req = builder.finish_owned(req);
        let response = client
            .say_hello(tonic::Request::new(OwnedHelloRequest::from(owned_req)))
            .await
            .unwrap();
        let reply = response.into_inner();
        let message = reply.get_ref().message();
        assert_eq!(message.unwrap(), "Hello, world3");

        // shutdown server
        token.cancel();
        svh.await.unwrap();
    }
}
