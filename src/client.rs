use crate::data::ParamRow;
use iced::futures::{SinkExt, Stream, StreamExt};
use reqwest::header::{HeaderName, HeaderValue};
use std::pin::Pin;
use std::time::{Duration, Instant};
use tokio::sync::mpsc::UnboundedSender;
use tokio_tungstenite::{connect_async, tungstenite::Message as WsMessage};

// shortcut: responses stop at 2 MiB, raise the cap when inspecting larger payloads becomes necessary.
const MAX_RESPONSE_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct RestInput {
    pub method: String,
    pub url: String,
    pub params: Vec<ParamRow>,
    pub headers: String,
    pub body: String,
}

#[derive(Debug, Clone)]
pub struct RestOutput {
    pub status: String,
    pub elapsed_ms: u128,
    pub body: String,
}

pub async fn send_rest(input: RestInput) -> Result<RestOutput, String> {
    let url =
        reqwest::Url::parse(input.url.trim()).map_err(|error| format!("URL inválida: {error}"))?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err("La URL debe usar http:// o https:// y tener un host.".into());
    }
    let method = reqwest::Method::from_bytes(input.method.as_bytes())
        .map_err(|error| format!("Método HTTP inválido: {error}"))?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|error| format!("No pude iniciar el cliente HTTP: {error}"))?;
    let mut request = client.request(method, url);
    for param in input
        .params
        .iter()
        .filter(|param| param.enabled && !param.name.trim().is_empty())
    {
        request = request.query(&[(param.name.trim(), param.value.as_str())]);
    }
    for (name, value) in parse_pairs(&input.headers, ':') {
        let name = HeaderName::from_bytes(name.trim().as_bytes())
            .map_err(|error| format!("Nombre de cabecera inválido: {error}"))?;
        let value = HeaderValue::from_str(value.trim())
            .map_err(|error| format!("Valor de cabecera inválido: {error}"))?;
        request = request.header(name, value);
    }
    if !input.body.is_empty() {
        request = request.body(input.body);
    }

    let started = Instant::now();
    let mut response = request
        .send()
        .await
        .map_err(|error| format!("Error de petición: {error}"))?;
    let status = response.status().to_string();
    let mut bytes = Vec::new();
    let mut truncated = false;
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| format!("No pude leer la respuesta: {error}"))?
    {
        let remaining = MAX_RESPONSE_BYTES.saturating_sub(bytes.len());
        bytes.extend_from_slice(&chunk[..chunk.len().min(remaining)]);
        if chunk.len() > remaining {
            truncated = true;
            break;
        }
    }
    let mut body = String::from_utf8_lossy(&bytes).into_owned();
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) {
        if let Ok(pretty) = serde_json::to_string_pretty(&json) {
            body = pretty;
        }
    }
    if truncated {
        body.push_str("\n\n[Respuesta recortada tras 2 MiB]");
    }
    Ok(RestOutput {
        status,
        elapsed_ms: started.elapsed().as_millis(),
        body,
    })
}

fn parse_pairs(input: &str, separator: char) -> Vec<(String, String)> {
    input
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| {
            line.split_once(separator)
                .map(|(key, value)| (key.trim().to_owned(), value.trim().to_owned()))
                .unwrap_or_else(|| (line.to_owned(), String::new()))
        })
        .filter(|(key, _)| !key.is_empty())
        .collect()
}

#[derive(Debug, Clone)]
pub enum SocketEvent {
    Connected {
        id: u64,
        sender: UnboundedSender<String>,
    },
    Received {
        id: u64,
        text: String,
    },
    Error {
        id: u64,
        text: String,
    },
    Closed {
        id: u64,
    },
}

pub fn socket_worker(
    data: &(u64, String, String),
) -> Pin<Box<dyn Stream<Item = SocketEvent> + Send + 'static>> {
    let (id, url, headers) = data.clone();
    Box::pin(iced::stream::channel(100, async move |mut output| {
        let mut request = match url.trim().into_client_request() {
            Ok(request) => request,
            Err(error) => {
                let _ = output
                    .send(SocketEvent::Error {
                        id,
                        text: format!("URL de WebSocket inválida: {error}"),
                    })
                    .await;
                return;
            }
        };
        let parsed = match reqwest::Url::parse(url.trim()) {
            Ok(url) if matches!(url.scheme(), "ws" | "wss") && url.host_str().is_some() => url,
            Ok(_) => {
                let _ = output
                    .send(SocketEvent::Error {
                        id,
                        text: "La URL debe usar ws:// o wss:// y tener un host.".into(),
                    })
                    .await;
                return;
            }
            Err(error) => {
                let _ = output
                    .send(SocketEvent::Error {
                        id,
                        text: format!("URL de WebSocket inválida: {error}"),
                    })
                    .await;
                return;
            }
        };
        let _ = parsed;
        for (name, value) in parse_pairs(&headers, ':') {
            let name = match HeaderName::from_bytes(name.trim().as_bytes()) {
                Ok(name) => name,
                Err(error) => {
                    let _ = output
                        .send(SocketEvent::Error {
                            id,
                            text: format!("Nombre de cabecera inválido: {error}"),
                        })
                        .await;
                    return;
                }
            };
            let value = match HeaderValue::from_str(value.trim()) {
                Ok(value) => value,
                Err(error) => {
                    let _ = output
                        .send(SocketEvent::Error {
                            id,
                            text: format!("Valor de cabecera inválido: {error}"),
                        })
                        .await;
                    return;
                }
            };
            request.headers_mut().insert(name, value);
        }

        let (socket, response) =
            match tokio::time::timeout(Duration::from_secs(30), connect_async(request)).await {
                Ok(Ok(connection)) => connection,
                Ok(Err(error)) => {
                    let _ = output
                        .send(SocketEvent::Error {
                            id,
                            text: format!("No pude conectar: {error}"),
                        })
                        .await;
                    return;
                }
                Err(_) => {
                    let _ = output
                        .send(SocketEvent::Error {
                            id,
                            text: "La conexión agotó el tiempo de espera (30 s).".into(),
                        })
                        .await;
                    return;
                }
            };
        let _ = response;
        let (sender, mut outgoing) = tokio::sync::mpsc::unbounded_channel();
        if output
            .send(SocketEvent::Connected { id, sender })
            .await
            .is_err()
        {
            return;
        }
        let (mut sink, mut incoming) = socket.split();
        loop {
            tokio::select! {
                item = incoming.next() => match item {
                    Some(Ok(WsMessage::Text(text))) => {
                        if output.send(SocketEvent::Received { id, text: text.to_string() }).await.is_err() { break; }
                    }
                    Some(Ok(WsMessage::Binary(bytes))) => {
                        let text = String::from_utf8_lossy(&bytes).into_owned();
                        if output.send(SocketEvent::Received { id, text }).await.is_err() { break; }
                    }
                    Some(Ok(WsMessage::Ping(bytes))) => {
                        if sink.send(WsMessage::Pong(bytes)).await.is_err() { break; }
                    }
                    Some(Ok(WsMessage::Close(frame))) => {
                        let _ = sink.send(WsMessage::Close(frame)).await;
                        break;
                    }
                    None => break,
                    Some(Ok(_)) => {}
                    Some(Err(error)) => {
                        let _ = output.send(SocketEvent::Error { id, text: format!("Error de WebSocket: {error}") }).await;
                        break;
                    }
                },
                message = outgoing.recv() => match message {
                    Some(text) => if let Err(error) = sink.send(WsMessage::Text(text.into())).await {
                        let _ = output.send(SocketEvent::Error { id, text: format!("No pude enviar: {error}") }).await;
                        break;
                    },
                    None => {
                        let _ = sink.send(WsMessage::Close(None)).await;
                        break;
                    }
                }
            }
        }
        let _ = output.send(SocketEvent::Closed { id }).await;
    }))
}

use tokio_tungstenite::tungstenite::client::IntoClientRequest;
