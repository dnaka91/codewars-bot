use log::{info, warn};
use tokio::signal;
use tokio::stream::StreamExt;
use tokio::sync::mpsc::UnboundedSender;
use warp::Filter;

use self::handlers::State;
use crate::api::slack::event::AppMention;

pub async fn run(port: u16, signing_key: String, sender: UnboundedSender<AppMention>) {
    let routes = filters::index()
        .or(filters::event(State {
            signing_key,
            sender,
        }))
        .with(warp::log("server"));

    let (addr, server) =
        warp::serve(routes).bind_with_graceful_shutdown(([0, 0, 0, 0], port), shutdown_signal());

    info!("listening on {}", addr);
    server.await
}

async fn shutdown_signal() {
    #[cfg(unix)]
    let mut signals = {
        use tokio::signal::unix::{signal, SignalKind};
        if let Ok(s) = signal(SignalKind::terminate()) {
            s
        } else {
            warn!("failed to install terminate signal handler");
            return;
        }
    };
    #[cfg(not(unix))]
    let mut signals = tokio::stream::pending::<()>();

    tokio::select! {
        _ = signals.next() => (),
        s = signal::ctrl_c() => {
            if s.is_err() {
                warn!("failed to install CTRL+C signal handler");
            }
        }
    }

    info!("shutting down");
}

mod filters {
    use std::convert::Infallible;

    use warp::Filter;

    use super::handlers::{self, State};

    pub fn index() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::get().and(warp::path::end()).map(handlers::index)
    }

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

    fn with_state(state: State) -> impl Filter<Extract = (State,), Error = Infallible> + Clone {
        warp::any().map(move || state.clone())
    }
}

mod handlers {
    #![allow(clippy::needless_pass_by_value)]

    use anyhow::Result;
    use bytes::Bytes;
    use log::{error, info, trace};
    use tokio::sync::mpsc::UnboundedSender;
    use warp::http::header;
    use warp::http::StatusCode;

    use crate::api::slack::event::{self, AppMention, Callback, Event};

    const INDEX_HTML: &[u8] = include_bytes!("index.html");

    #[derive(Debug, Clone)]
    pub struct State {
        pub signing_key: String,
        pub sender: UnboundedSender<AppMention>,
    }

    pub fn index() -> impl warp::Reply {
        warp::reply::html(INDEX_HTML)
    }

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

    pub fn error(resp: Result<Option<String>>) -> impl warp::Reply {
        let (status, content) = match resp {
            Ok(opt) => (
                StatusCode::OK,
                match opt {
                    Some(value) => value,
                    None => String::new(),
                },
            ),
            Err(e) => {
                error!("Error during event processing: {:?}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, String::new())
            }
        };

        warp::reply::with_status(
            warp::reply::with_header(content, header::CONTENT_TYPE, "text/plain"),
            status,
        )
    }
}
