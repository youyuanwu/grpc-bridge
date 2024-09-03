use hyper::body::Incoming;
use hyper_util::client::legacy::{Client, ResponseFuture};

use crate::conn::UdsConnector;
use tower::Service;

// Proxy hyper service for uds.
// Pipes all hyper requests into the uds client
#[derive(Debug, Clone)]
pub struct UdsService {
    c: Client<UdsConnector, Incoming>,
}

impl UdsService {
    pub async fn new(conn: UdsConnector) -> Self {
        Self {
            c: make_client(conn).await,
        }
    }
}

impl hyper::service::Service<hyper::Request<Incoming>> for UdsService {
    type Response = hyper::Response<Incoming>;

    type Error = hyper_util::client::legacy::Error;

    type Future = ResponseFuture;

    fn call(&self, req: hyper::Request<Incoming>) -> Self::Future {
        let mut c = self.c.clone();
        c.call(req)
    }
}

async fn make_client<R: hyper::body::Body + Send>(conn: UdsConnector) -> Client<UdsConnector, R>
where
    <R as hyper::body::Body>::Data: Send,
{
    hyper_util::client::legacy::Builder::new(hyper_util::rt::TokioExecutor::new())
        .pool_idle_timeout(std::time::Duration::from_secs(3))
        .pool_timer(hyper_util::rt::TokioTimer::new())
        .http2_only(true)
        .build::<_, R>(conn)
}
