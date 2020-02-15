use futures::channel::mpsc::{self, UnboundedReceiver, UnboundedSender};
use futures::prelude::*;
use log::trace;
use serde::Deserialize;
use serde_json::Value;
use tokio_tungstenite::tungstenite::Message;

use super::Result;

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum Event {
    Hello,
    Error {
        code: i32,
        msg: String,
    },
    Message {
        channel: String,
        user: String,
        text: String,
        ts: String,
    },
}

pub async fn connect() -> Result<(UnboundedSender<Value>, UnboundedReceiver<Event>)> {
    let url = super::web::rtm_connect().await?;

    let (ws, _) = tokio_tungstenite::connect_async(&url).await?;
    let (write, mut read) = ws.split();

    let (tx, rx) = mpsc::unbounded();

    tokio::spawn(rx.map(Ok).forward(write));

    let (value_tx, value_rx) = mpsc::unbounded();
    let (event_tx, event_rx) = mpsc::unbounded();

    tokio::spawn(
        value_rx
            .map(|v: Value| Message::Text(serde_json::to_string(&v).unwrap()))
            .map(Ok)
            .forward(tx.clone()),
    );

    tokio::spawn(async move {
        while let Some(message) = read.try_next().await.unwrap() {
            match message {
                Message::Text(msg) => {
                    let message = serde_json::from_str::<Value>(&msg).unwrap();
                    let raw_msg = message.as_object().unwrap();
                    let msg_type = raw_msg.get("type").unwrap().as_str().unwrap();
                    let types = &["hello", "error", "message"];

                    if types.contains(&msg_type) && message.get("subtype").is_none() {
                        let event = serde_json::from_value::<Event>(message).unwrap();
                        trace!("TEXT {:?}", event);
                        event_tx.unbounded_send(event).unwrap();
                    } else {
                        trace!(
                            "unsupported event `{}` (subtype {:?})",
                            msg_type,
                            message.get("subtype").map(Value::as_str).flatten()
                        );
                    }
                }
                Message::Binary(msg) => trace!("BINARY {:?}", msg),
                Message::Ping(msg) => {
                    trace!("PING {:?}", msg);
                    tx.unbounded_send(Message::Pong(msg)).unwrap();
                }
                Message::Pong(msg) => trace!("PONG {:?}", msg),
                Message::Close(msg) => trace!("CLOSE {:?}", msg),
            }
        }
    });

    Ok((value_tx, event_rx))
}
