use serde::Deserialize;
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

pub async fn run_server() {
    let routes = filters::index()
        .or(filters::event())
        .with(warp::log("server"));

    warp::serve(routes).run(([127, 0, 0, 1], 8080)).await
}

mod filters {
    use warp::Filter;

    use super::handlers;

    pub fn index() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::get().and(warp::path::end()).map(handlers::index)
    }

    pub fn event() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::post()
            .and(warp::path!("event"))
            .and(warp::body::json())
            .map(handlers::event)
            .map(handlers::error)
    }
}

mod handlers {
    #![allow(clippy::needless_pass_by_value)]

    use anyhow::{anyhow, Result};
    use log::{error, info};
    use serde_json::Value;
    use warp::http::header;
    use warp::http::StatusCode;

    use super::{AppMention, UrlVerification};

    const INDEX_HTML: &[u8] = include_bytes!("index.html");

    const TYPE_URL_VERIFICATION: &str = "url_verification";
    const TYPE_APP_MENTION: &str = "app_mention";

    pub fn index() -> impl warp::Reply {
        warp::reply::html(INDEX_HTML)
    }

    pub fn event(event: Value) -> Result<Option<String>> {
        match event
            .get("type")
            .ok_or_else(|| anyhow!("missing `type` property"))?
            .as_str()
            .ok_or_else(|| anyhow!("type is not a string"))?
        {
            TYPE_URL_VERIFICATION => {
                let event: UrlVerification = serde_json::from_value(event)?;
                Ok(Some(event.challenge))
            }
            TYPE_APP_MENTION => {
                let event: AppMention = serde_json::from_value(event)?;
                tokio::spawn(async move {
                    info!(target:"server", "{:?}", event);
                });
                Ok(None)
            }
            event => {
                info!(target: "server", "Got unknown event type `{}`", event);
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
