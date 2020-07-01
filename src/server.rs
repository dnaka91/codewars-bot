//! Implementation of a HTTP server to listen for message events from Slack. It also features a
//! landing page to introduce features of the service.

use log::{info, warn};
use tokio::sync::mpsc::UnboundedSender;
use warp::Filter;

use self::handlers::State;
use crate::api::slack::event::AppMention;

/// Run the server on the given port. A signing key is required to verify events come from Slack and
/// any successfully parsed events are sent back through the given sender.
pub async fn run(port: u16, signing_key: String, sender: UnboundedSender<AppMention>) {
    let routes = filters::index()
        .or(filters::favicon())
        .or(filters::event(State {
            signing_key,
            sender,
        }))
        .map(filters::with_sec_headers)
        .with(warp::log("server"));

    let (addr, server) =
        warp::serve(routes).bind_with_graceful_shutdown(([0, 0, 0, 0], port), shutdown_signal());

    info!("listening on {}", addr);
    server.await
}

/// The signal to wait for that triggers a shutdown of the server.
async fn shutdown_signal() {
    if tokio::signal::ctrl_c().await.is_err() {
        warn!("failed to install CTRL+C signal handler");
    }

    info!("shutting down");
}

mod filters {
    //! All the routes that this server supports.

    use std::convert::Infallible;

    use warp::Filter;

    use super::handlers::{self, State};

    /// Landing page at `/` with usage information.
    pub fn index() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::get().and(warp::path::end()).map(handlers::index)
    }

    /// Favicon for the landing page in different resolutions.
    pub fn favicon() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        favicon_16().or(favicon_32())
    }

    /// Favicon in 16x16px.
    fn favicon_16() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::get().and(warp::path!("favicon-16x16.png").map(handlers::favicon_16))
    }

    /// Favicon in 32x32px.
    fn favicon_32() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::get().and(warp::path!("favicon-32x32.png").map(handlers::favicon_32))
    }

    /// Endpoint at `/event` that receives Slack events.
    pub fn event(
        state: State,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::post()
            .and(warp::path!("event"))
            .and(warp::header("x-slack-signature"))
            .and(warp::header("x-slack-request-timestamp"))
            .and(warp::body::content_length_limit(1024 * 5))
            .and(warp::body::bytes())
            .and(with_state(state))
            .map(handlers::event)
            .map(handlers::error)
    }

    /// Attach the [`State`] to an existing filter.
    fn with_state(state: State) -> impl Filter<Extract = (State,), Error = Infallible> + Clone {
        warp::any().map(move || state.clone())
    }

    /// List of security headers that should be applied to all responses.
    const SEC_HEADERS: &[(&str, &str)] = &[
        ("referrer-policy", "same-origin"),
        (
            "strict-transport-security",
            "max-age=63072000; includeSubDomains; preload",
        ),
        ("x-content-type-options", "nosniff"),
        ("x-frame-options", "DENY"),
        ("x-xss-protection", "1; mode=block"),
    ];

    /// Wrap the a reply and add additional security headers to it, overwriting any existing header
    /// values if they already existed.
    pub fn with_sec_headers(reply: impl warp::Reply) -> impl warp::Reply {
        use warp::http::HeaderValue;

        let mut res = reply.into_response();
        let headers = res.headers_mut();

        for (k, v) in SEC_HEADERS.iter() {
            headers.insert(*k, HeaderValue::from_static(v));
        }

        res
    }
}

mod handlers {
    //! Handlers to the routes that implement the functionality of endpoints.

    #![allow(clippy::needless_pass_by_value)]

    use anyhow::Result;
    use bytes::Bytes;
    use log::{error, info, trace};
    use tokio::sync::mpsc::UnboundedSender;
    use warp::http::header;
    use warp::http::{Response, StatusCode};

    use crate::api::slack::event::{self, AppMention, Callback, Event};

    /// Static HTML of the index page.
    const INDEX_HTML: &[u8] = include_bytes!("../assets/index.html");

    /// Favicon image in 16x16px.
    const FAVICON_16X16_PNG: &[u8] = include_bytes!("../assets/favicon-16x16.png");
    /// Favicon image in 32x32px.
    const FAVICON_32X32_PNG: &[u8] = include_bytes!("../assets/favicon-32x32.png");
    /// Value for the `Cache-Control` header of favicon responses.
    const FAVICON_CACHE_CONTROL: &str = "public, max-age=2592000";

    /// The state carries some information shared by all instances of the event handler endpoint
    /// to properly handle Slack events.
    #[derive(Debug, Clone)]
    pub struct State {
        /// Key to verify events really come from Slack.
        pub signing_key: String,
        /// Channel to send back successfully parsed messages.
        pub sender: UnboundedSender<AppMention>,
    }

    /// Landing page with usage instructions.
    pub fn index() -> impl warp::Reply {
        warp::reply::html(INDEX_HTML)
    }

    /// Favicon in 16x16px.
    pub fn favicon_16() -> impl warp::Reply {
        Response::builder()
            .header(header::CONTENT_TYPE, "image/png")
            .header(header::CACHE_CONTROL, FAVICON_CACHE_CONTROL)
            .body(FAVICON_16X16_PNG)
    }

    /// Favicon in 32x32px.
    pub fn favicon_32() -> impl warp::Reply {
        Response::builder()
            .header(header::CONTENT_TYPE, "image/png")
            .header(header::CACHE_CONTROL, FAVICON_CACHE_CONTROL)
            .body(FAVICON_32X32_PNG)
    }

    /// Event endpoint that handles message events from Slack.
    pub fn event(
        signature: String,
        timestamp: String,
        body: Bytes,
        state: State,
    ) -> Result<Option<String>> {
        event::verify_signature(state.signing_key.as_bytes(), &signature, &timestamp, &body)?;

        let content = serde_json::from_slice(&body)?;

        match event::parse_callback(content)? {
            Callback::UrlVerification(uv) => {
                trace!("Received URL verification request");
                Ok(Some(uv.challenge))
            }
            Callback::Event(value) => {
                match event::parse_event(value)? {
                    Event::AppMention(am) => {
                        trace!("Received app mention event");
                        tokio::spawn(async move {
                            trace!("{:?}", am);
                            state.sender.send(am).unwrap();
                        });
                    }
                    Event::Unknown(name) => info!("Received unknown event ({})", name),
                }

                Ok(None)
            }
            Callback::Unknown(name) => {
                info!("Received unknown callback request ({})", name);
                Ok(None)
            }
        }
    }

    /// Error wrapper that turns any [`Result`]<[`Option`]<`T`>> into a proper HTTP response. The
    /// contained value must be a [`warp::Reply`] and have a default value.
    pub fn error<T>(resp: Result<Option<T>>) -> impl warp::Reply
    where
        T: Default + warp::Reply,
    {
        let (status, content) = match resp {
            Ok(opt) => (StatusCode::OK, opt.unwrap_or_default()),
            Err(e) => {
                error!("Error during event processing: {:?}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, Default::default())
            }
        };

        warp::reply::with_status(
            warp::reply::with_header(content, header::CONTENT_TYPE, "text/plain"),
            status,
        )
    }
}
