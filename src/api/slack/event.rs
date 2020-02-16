use serde::Deserialize;
use tokio::sync::mpsc::UnboundedSender;
use warp::Filter;

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

pub async fn run_server(sender: UnboundedSender<AppMention>) {
    let routes = filters::index()
        .or(filters::event(sender))
        .with(warp::log("server"));

    warp::serve(routes).run(([127, 0, 0, 1], 8080)).await
}

mod filters {
    use tokio::sync::mpsc::UnboundedSender;
    use warp::Filter;

    use super::handlers;
    use super::AppMention;

    pub fn index() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::get().and(warp::path::end()).map(handlers::index)
    }

    pub fn event(
        sender: UnboundedSender<AppMention>,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::post()
            .and(warp::path!("event"))
            .and(warp::body::json())
            .and(warp::any().map(move || sender.clone()))
            .map(handlers::event)
            .map(handlers::error)
    }
}

mod handlers {
    #![allow(clippy::needless_pass_by_value)]

    use anyhow::{anyhow, Result};
    use log::{error, info, trace};
    use serde_json::Value;
    use tokio::sync::mpsc::UnboundedSender;
    use warp::http::header;
    use warp::http::StatusCode;

    use super::{AppMention, UrlVerification};

    const INDEX_HTML: &[u8] = include_bytes!("index.html");

    const CALLBACK_URL_VERIFICATION: &str = "url_verification";
    const CALLBACK_EVENT_CALLBACK: &str = "event_callback";
    const EVENT_APP_MENTION: &str = "app_mention";

    pub fn index() -> impl warp::Reply {
        warp::reply::html(INDEX_HTML)
    }

    pub fn event(mut event: Value, sender: UnboundedSender<AppMention>) -> Result<Option<String>> {
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
                            sender.send(event).unwrap();
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
}
