use crate::whisper_service::WhisperService;
use anyhow::{Context, Result};
use hyper::body::to_bytes;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Method, Request, Response, Server, StatusCode};
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::{Arc, RwLock, OnceLock};
use std::time::Instant;

static WHISPER_SVC: OnceLock<Arc<RwLock<Option<Arc<WhisperService>>>>> = OnceLock::new();

pub fn service_holder() -> Arc<RwLock<Option<Arc<WhisperService>>>> {
    WHISPER_SVC
        .get_or_init(|| Arc::new(RwLock::new(None)))
        .clone()
}

pub fn load_model(model_path: String) -> Result<()> {
    let holder = service_holder();
    let svc = Arc::new(WhisperService::from_model_path(&model_path).context("load model")?);
    if let Ok(mut w) = holder.write() {
        *w = Some(svc);
    }
    Ok(())
}

pub fn is_model_loaded() -> bool {
    let holder = service_holder();
    {
        if let Ok(r) = holder.read() {
            return r.is_some();
        }
    }
    false
}

async fn handle(
    holder: Arc<RwLock<Option<Arc<WhisperService>>>>,
    req: Request<Body>,
) -> Result<Response<Body>, Response<Body>> {
    if req.method() == Method::POST && req.uri().path() == "/transcribe" {
        // Require model to be loaded
        let svc = {
            let guard = holder.read().map_err(|_| {
                let mut resp = Response::new(Body::from("internal lock error"));
                *resp.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
                resp
            })?;
            guard.clone()
        };
        let svc = match svc {
            Some(s) => s,
            None => {
                let body = serde_json::json!({
                    "error": "model_not_loaded",
                    "message": "No Whisper model is loaded. Please download and select a model.",
                }).to_string();
                let mut resp = Response::new(Body::from(body));
                *resp.status_mut() = StatusCode::SERVICE_UNAVAILABLE;
                resp.headers_mut().insert(
                    hyper::header::CONTENT_TYPE,
                    hyper::header::HeaderValue::from_static("application/json"),
                );
                return Err(resp);
            }
        };

        let content_type = req
            .headers()
            .get(hyper::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");

        if !content_type.contains("audio/wav") && !content_type.contains("application/octet-stream") {
            let mut resp = Response::new(Body::from("unsupported Content-Type; send audio/wav"));
            *resp.status_mut() = StatusCode::UNSUPPORTED_MEDIA_TYPE;
            return Err(resp);
        }

        // Read optional language from query string: /transcribe?lang=en
        let language: Option<String> = req
            .uri()
            .query()
            .and_then(|q| {
                // simple parse for lang=...
                q.split('&')
                    .find_map(|pair| {
                        let mut it = pair.splitn(2, '=');
                        match (it.next(), it.next()) {
                            (Some("lang"), Some(val)) if !val.is_empty() => Some(val.to_string()),
                            _ => None,
                        }
                    })
            });
        // Read optional initial prompt from query string: /transcribe?prompt=...
        let initial_prompt: Option<String> = req
            .uri()
            .query()
            .and_then(|q| {
                q.split('&')
                    .find_map(|pair| {
                        let mut it = pair.splitn(2, '=');
                        match (it.next(), it.next()) {
                            (Some("prompt"), Some(val)) if !val.is_empty() => {
                                // Keep as-is (already percent-encoded by client)
                                Some(percent_encoding::percent_decode_str(val).decode_utf8_lossy().to_string())
                            }
                            _ => None,
                        }
                    })
            });
        let t_req_total = Instant::now();
        let t_read_start = Instant::now();
        let body_bytes = to_bytes(req.into_body()).await.map_err(|e| {
            let mut resp = Response::new(Body::from(format!("failed to read body: {}", e)));
            *resp.status_mut() = StatusCode::BAD_REQUEST;
            resp
        })?;
        let read_body_ms = t_read_start.elapsed().as_millis();

        let t_transcribe_start = Instant::now();
        let (text, timings) = svc
            .transcribe_wav_bytes_profiled(&body_bytes, language.as_deref(), initial_prompt.as_deref())
            .map_err(|e| {
                let mut resp = Response::new(Body::from(format!("transcription error: {}", e)));
                *resp.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
                resp
            })?;
        let transcribe_ms = t_transcribe_start.elapsed().as_millis();

        let total_req_ms = t_req_total.elapsed().as_millis();

        let backend = svc.backend_info();
        let body = serde_json::json!({
            "text": text,
            "profile": {
                "server": {
                    "read_body_ms": read_body_ms,
                    "transcribe_ms": transcribe_ms,
                    "total_ms": total_req_ms,
                    "backend": backend
                },
                "whisper": timings
            }
        }).to_string();
        let mut resp = Response::new(Body::from(body));
        resp.headers_mut().insert(
            hyper::header::CONTENT_TYPE,
            hyper::header::HeaderValue::from_static("application/json"),
        );
        return Ok(resp);
    }

    let mut resp = Response::new(Body::from("Not Found"));
    *resp.status_mut() = StatusCode::NOT_FOUND;
    Err(resp)
}

pub fn start_whisper_server(bind_addr: String) -> Result<()> {
    let holder = service_holder();

    std::thread::spawn(move || {
        let rt = match tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(e) => {
                eprintln!("failed to build tokio runtime: {}", e);
                return;
            }
        };

        rt.block_on(async move {
            let addr: SocketAddr = match bind_addr.parse() {
                Ok(a) => a,
                Err(e) => {
                    eprintln!("invalid bind addr {}: {}", bind_addr, e);
                    return;
                }
            };

            let make_svc = {
                let holder = holder.clone();
                make_service_fn(move |_conn| {
                    let holder = holder.clone();
                    async move {
                        Ok::<_, Infallible>(service_fn(move |req| {
                            let holder = holder.clone();
                            async move {
                                match handle(holder, req).await {
                                    Ok(resp) => Ok::<_, Infallible>(resp),
                                    Err(resp) => Ok::<_, Infallible>(resp),
                                }
                            }
                        }))
                    }
                })
            };

            let server = Server::bind(&addr).serve(make_svc);
            if let Err(e) = server.await {
                eprintln!("hyper server error: {}", e);
            }
        });
    });

    Ok(())
}


