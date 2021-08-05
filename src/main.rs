use anyhow::Result;
use axum::prelude::*;
use bytes::Bytes;
use http::{header, HeaderValue, Uri};
use std::net::SocketAddr;
use std::time::Duration;
use structopt::StructOpt;
use tower::ServiceBuilder;
use tower_http::compression::CompressionLayer;
use tower_http::sensitive_headers::SetSensitiveHeadersLayer;
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::trace::TraceLayer;

#[derive(Debug, StructOpt)]
struct Config {
    #[structopt(long, short = "p", default_value = "3000")]
    port: u16,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .json()
        .with_timer(tracing_subscriber::fmt::time::ChronoUtc::rfc3339())
        .with_current_span(true)
        .init();

    let config = Config::from_args();

    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));

    serve_forever(addr).await.expect("server error");
}

async fn serve_forever(addr: SocketAddr) -> Result<()> {
    let middleware_stack = ServiceBuilder::new()
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(|_: &Request<Body>| {
                    tracing::info_span!("http-request")
                })
                .on_body_chunk(|chunk: &Bytes, latency: Duration, span: &tracing::Span| {
                    tracing::info!(size_bytes = chunk.len(), latency_micro = latency.as_micros() as usize, span_id = ?span.id(), "sending body chunk")
                })
        )
        .timeout(Duration::from_secs(10))
        // .handle_error(|error: BoxError| {
        //     // Check if the actual error type is `Elapsed` which
        //     // `Timeout` returns
        //     if error.is::<Elapsed>() {
        //         return Ok::<_, Infallible>((
        //             StatusCode::REQUEST_TIMEOUT,
        //             "Request took too long".into(),
        //         ));
        //     }
        //
        //     // If we encounter some error we don't handle return a generic
        //     // error
        //     return Ok::<_, Infallible>((
        //         StatusCode::INTERNAL_SERVER_ERROR,
        //         // `Cow` lets us return either `&str` or `String`
        //         Cow::from(format!("Unhandled internal error: {}", error)),
        //     ));
        // })
        .layer(CompressionLayer::new())
        .layer(SetResponseHeaderLayer::<_, Request<Body>>::if_not_present(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/plain"),
        ))
        .layer(SetSensitiveHeadersLayer::new(vec![
            header::AUTHORIZATION,
            header::COOKIE,
        ]))
        .into_inner();

    let routes = route("/:name", get(hello)).layer(middleware_stack);

    tracing::info!("Listening on {}", addr);

    hyper::Server::bind(&addr)
        .serve(routes.into_make_service())
        .await?;

    Ok(())
}

async fn hello(uri: Uri) -> String {
    format!("hello {}", uri.path())
}
