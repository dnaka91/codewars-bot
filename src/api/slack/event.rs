use log::{info, warn};
use serde::Deserialize;
use tokio::signal;
use tokio::stream::StreamExt;
use tokio::sync::mpsc::UnboundedSender;
use warp::Filter;

use self::handlers::State;

#[derive(Debug, Deserialize)]
pub struct UrlVerification {
    pub challenge: String,
}

#[derive(Debug, Deserialize)]
pub struct AppMention {
    pub user: String,
    pub text: String,
    pub channel: String,
}

pub async fn run_server(signing_key: String, sender: UnboundedSender<AppMention>) {
    let routes = filters::index()
        .or(filters::event(State {
            signing_key,
            sender,
        }))
        .with(warp::log("server"));

    let (addr, server) =
        warp::serve(routes).bind_with_graceful_shutdown(([0, 0, 0, 0], 8080), shutdown_signal());

    info!(target:"server", "listening on {}", addr);
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

    info!(target:"server", "shutting down");
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

    use anyhow::{anyhow, ensure, Result};
    use bytes::Bytes;
    use hmac::{Hmac, Mac};
    use log::{error, info, trace};
    use serde_json::Value;
    use sha2::Sha256;
    use tokio::sync::mpsc::UnboundedSender;
    use warp::http::header;
    use warp::http::StatusCode;

    use super::{AppMention, UrlVerification};

    const INDEX_HTML: &[u8] = include_bytes!("index.html");

    const CALLBACK_URL_VERIFICATION: &str = "url_verification";
    const CALLBACK_EVENT_CALLBACK: &str = "event_callback";
    const EVENT_APP_MENTION: &str = "app_mention";

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
        let mut event =
            verify_signature(state.signing_key.as_bytes(), signature, timestamp, &body)?;

        match event
            .get("type")
            .ok_or_else(|| anyhow!("missing `type` property"))?
            .as_str()
            .ok_or_else(|| anyhow!("type is not a string"))?
        {
            CALLBACK_URL_VERIFICATION => {
                trace!(target: "server", "Received URL verification request");
                let event: UrlVerification = serde_json::from_value(event)?;
                Ok(Some(event.challenge))
            }
            CALLBACK_EVENT_CALLBACK => {
                trace!(target: "server", "Received event callback request");
                let event = event
                    .get_mut("event")
                    .ok_or_else(|| anyhow!("missing `event` property"))?;

                match event
                    .as_object()
                    .ok_or_else(|| anyhow!("event is not an object"))?
                    .get("type")
                    .ok_or_else(|| anyhow!("missing `type` property"))?
                    .as_str()
                    .ok_or_else(|| anyhow!("type is not a string"))?
                {
                    EVENT_APP_MENTION => {
                        trace!(target: "server", "Received app mention event");
                        let event: AppMention = serde_json::from_value(event.take())?;
                        tokio::spawn(async move {
                            trace!(target:"server", "{:?}", event);
                            state.sender.send(event).unwrap();
                        });
                    }
                    event_type => {
                        info!(target: "server", "Received unknown event ({})", event_type)
                    }
                }

                Ok(None)
            }
            callback_type => {
                info!(target: "server", "Received unknown callback request ({})", callback_type);
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
                error!(target:"server", "Error during event processing: {:?}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, String::new())
            }
        };

        warp::reply::with_status(
            warp::reply::with_header(content, header::CONTENT_TYPE, "text/plain"),
            status,
        )
    }

    fn verify_signature(
        key: &[u8],
        signature: String,
        timestamp: String,
        body: &[u8],
    ) -> Result<Value> {
        ensure!(
            signature.starts_with("v0="),
            "unsupported signature version"
        );

        let sig_data = hex::decode(&signature[3..])?;

        let mut mac = Hmac::<Sha256>::new_varkey(key).map_err(|_| anyhow!("Invalid key size"))?;

        mac.input(b"v0:");
        mac.input(timestamp.as_bytes());
        mac.input(b":");
        mac.input(body);

        mac.verify(&sig_data).map_err(|e| anyhow!(e.to_string()))?;

        Ok(serde_json::from_slice(body)?)
    }
}
