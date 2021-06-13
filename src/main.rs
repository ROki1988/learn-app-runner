use bytes::Bytes;
use std::net::SocketAddr;
use std::net::TcpListener;
use std::time::Duration;
use structopt::StructOpt;
use tower::make::Shared;
use tower::ServiceBuilder;
use tower_http::compression::CompressionLayer;
use tower_http::sensitive_headers::SetSensitiveHeadersLayer;
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer};
use tower_http::LatencyUnit;
use warp::hyper::body::HttpBody;
use warp::hyper::header::HeaderValue;
use warp::hyper::{header, Body, Request, Response, Server};
use warp::path;
use warp::{Filter, Rejection, Reply};

#[derive(Debug, StructOpt)]
struct Config {
    #[structopt(long, short = "p", default_value = "3000")]
    port: u16,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let config = Config::from_args();

    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    let listener = TcpListener::bind(addr).unwrap();

    serve_forever(listener).await.expect("server error");
}

async fn serve_forever(listener: TcpListener) -> Result<(), warp::hyper::Error> {
    let filter = error().or(get());

    let warp_service = warp::service(filter);

    let service = ServiceBuilder::new()
        .layer(
            TraceLayer::new_for_http()
                .on_body_chunk(|chunk: &Bytes, latency: Duration, _: &tracing::Span| {
                    tracing::trace!(size_bytes = chunk.len(), latency = ?latency, "sending body chunk")
                })
                .make_span_with(DefaultMakeSpan::new().include_headers(true))
                .on_response(DefaultOnResponse::new().include_headers(true).latency_unit(LatencyUnit::Micros)),
        )
        .timeout(Duration::from_secs(10))
        .layer(CompressionLayer::new())
        .layer(SetResponseHeaderLayer::overriding(
            header::CONTENT_LENGTH,
            content_length_from_response,
        ))
        .layer(SetResponseHeaderLayer::<_, Request<Body>>::if_not_present(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/plain"),
        ))
        .layer(SetSensitiveHeadersLayer::new(vec![
            header::AUTHORIZATION,
            header::COOKIE,
        ]))
        .service(warp_service);

    // Run the service using hyper
    let addr = listener.local_addr().unwrap();

    tracing::info!("Listening on {}", addr);

    Server::from_tcp(listener)
        .unwrap()
        .serve(Shared::new(service))
        .await?;

    Ok(())
}

pub fn get() -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    warp::get()
        .and(path!(String))
        .map(|path: String| Response::new(Body::from("hello ".to_owned() + &path)))
}

pub fn error() -> impl Filter<Extract = (&'static str,), Error = Rejection> + Clone {
    warp::get()
        .and(path!("debug" / "error"))
        .and_then(|| async move { Err(warp::reject::custom(InternalError)) })
}

#[derive(Debug)]
struct InternalError;

impl warp::reject::Reject for InternalError {}

fn content_length_from_response<B>(response: &Response<B>) -> Option<HeaderValue>
where
    B: HttpBody,
{
    response
        .body()
        .size_hint()
        .exact()
        .map(|size| HeaderValue::from_str(&size.to_string()).unwrap())
}
