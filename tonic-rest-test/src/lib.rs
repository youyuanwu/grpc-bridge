pub mod greeter {
    tonic::include_proto!("greeter");
}

#[allow(unfulfilled_lint_expectations)]
mod rest_routes {
    include!(concat!(env!("OUT_DIR"), "/rest_routes.rs"));
}
pub use rest_routes::*;

use greeter::greeter_server::Greeter as GreeterTrait;
use greeter::{GetGreetingRequest, HelloReply, HelloRequest};

#[derive(Default, Clone)]
pub struct Greeter;

#[tonic::async_trait]
impl GreeterTrait for Greeter {
    async fn say_hello(
        &self,
        request: tonic::Request<HelloRequest>,
    ) -> Result<tonic::Response<HelloReply>, tonic::Status> {
        let name = request.into_inner().name;
        Ok(tonic::Response::new(HelloReply {
            message: format!("hello {name}"),
        }))
    }

    async fn get_greeting(
        &self,
        request: tonic::Request<GetGreetingRequest>,
    ) -> Result<tonic::Response<HelloReply>, tonic::Status> {
        let name = request.into_inner().name;
        Ok(tonic::Response::new(HelloReply {
            message: format!("hello {name}"),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use greeter::greeter_client::GreeterClient;
    use greeter::greeter_server::GreeterServer;
    use std::sync::Arc;
    use tokio_util::sync::CancellationToken;

    static SEM: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(1);

    /// Spawn a single Axum server that serves both the gRPC routes and the
    /// generated REST routes on the same port.
    async fn spawn_server(
        token: CancellationToken,
    ) -> (std::net::SocketAddr, tokio::task::JoinHandle<()>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let grpc = tonic::service::Routes::new(GreeterServer::new(Greeter)).into_axum_router();
        let rest = crate::greeter_rest_router(Arc::new(Greeter));
        let app = rest.merge(grpc);

        let handle = tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async move { token.cancelled().await })
                .await
                .unwrap();
        });

        (addr, handle)
    }

    #[tokio::test]
    async fn grpc_say_hello() {
        let _permit = SEM.acquire().await.unwrap();
        let token = CancellationToken::new();
        let (addr, svh) = spawn_server(token.clone()).await;

        let mut client = GreeterClient::connect(format!("http://{addr}"))
            .await
            .unwrap();
        let reply = client
            .say_hello(tonic::Request::new(HelloRequest {
                name: "world".into(),
            }))
            .await
            .unwrap()
            .into_inner();
        assert_eq!(reply.message, "hello world");

        token.cancel();
        svh.await.unwrap();
    }

    #[tokio::test]
    async fn rest_say_hello_post() {
        let _permit = SEM.acquire().await.unwrap();
        let token = CancellationToken::new();
        let (addr, svh) = spawn_server(token.clone()).await;

        let resp: serde_json::Value = reqwest::Client::new()
            .post(format!("http://{addr}/v1/sayhello"))
            .json(&serde_json::json!({ "name": "rest" }))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(resp["message"], "hello rest");

        token.cancel();
        svh.await.unwrap();
    }

    #[tokio::test]
    async fn rest_get_greeting() {
        let _permit = SEM.acquire().await.unwrap();
        let token = CancellationToken::new();
        let (addr, svh) = spawn_server(token.clone()).await;

        let resp: serde_json::Value = reqwest::get(format!("http://{addr}/v1/greeting/alice"))
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(resp["message"], "hello alice");

        token.cancel();
        svh.await.unwrap();
    }
}
