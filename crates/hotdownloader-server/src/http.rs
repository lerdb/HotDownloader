//! 独立进程的任务 HTTP 与 SSE 入口。连接只借用进程级运行时，不拥有下载任务。

use std::convert::Infallible;
use std::path::{Component, Path};
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use futures_util::stream;
use hotdownloader_core::contract::CreateTaskRequest;
use hotdownloader_core::qq_login::{self, LoginCredentialStore};
use hotdownloader_core::task_service::TaskService;
use http_body_util::{BodyExt, Full, Limited, StreamBody};
use hyper::body::{Frame, Incoming};
use hyper::{Method, Request, Response, StatusCode};
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::sync::mpsc;

use crate::music::{self, MusicRequest};
use crate::runtime::ServerRuntime;

type HttpBody = http_body_util::combinators::UnsyncBoxBody<Bytes, Infallible>;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeleteRequest {
    #[serde(default)]
    delete_file: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManualLoginRequest {
    uin: String,
    authst: String,
    refresh_token: Option<String>,
    refresh_key: Option<String>,
    access_token: Option<String>,
    openid: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BatchRemoveRequest {
    task_ids: Vec<String>,
    #[serde(default)]
    delete_file: bool,
}

fn json_response(status: StatusCode, value: Value) -> Response<HttpBody> {
    let bytes = serde_json::to_vec(&value).unwrap_or_else(|_| b"{}".to_vec());
    Response::builder()
        .status(status)
        .header("content-type", "application/json; charset=utf-8")
        .header("cache-control", "no-store")
        .body(Full::new(Bytes::from(bytes)).boxed_unsync())
        .expect("固定的 HTTP 响应头应始终有效")
}

fn error_response(status: StatusCode, message: impl AsRef<str>) -> Response<HttpBody> {
    json_response(status, json!({ "error": message.as_ref() }))
}

fn static_response(web_dir: &Path, url_path: &str) -> Response<HttpBody> {
    let relative = if url_path == "/" {
        "index.html"
    } else {
        url_path.trim_start_matches('/')
    };
    let path = Path::new(relative);
    // 仅允许 Vite 产物中的普通相对路径，阻止请求读取数据目录和进程文件。
    if relative.contains('%')
        || relative.contains('\\')
        || path
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        return error_response(StatusCode::NOT_FOUND, "静态资源不存在");
    }
    let file = web_dir.join(path);
    let bytes = match std::fs::read(&file) {
        Ok(bytes) => bytes,
        Err(_) => return error_response(StatusCode::NOT_FOUND, "静态资源不存在"),
    };
    let content_type = match file.extension().and_then(|ext| ext.to_str()).unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "js" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        _ => "application/octet-stream",
    };
    Response::builder()
        .status(StatusCode::OK)
        .header("content-type", content_type)
        .header("x-content-type-options", "nosniff")
        .header("referrer-policy", "same-origin")
        .body(Full::new(Bytes::from(bytes)).boxed_unsync())
        .expect("固定的静态资源响应头应始终有效")
}

async fn read_json(request: Request<Incoming>) -> Result<Value, String> {
    // 限制 API 请求体大小，避免误上传大文件占满服务进程内存。
    let body = Limited::new(request.into_body(), 1024 * 1024);
    let bytes = body
        .collect()
        .await
        .map_err(|error| format!("读取请求体失败: {error}"))?
        .to_bytes();
    serde_json::from_slice(&bytes).map_err(|error| format!("解析 JSON 失败: {error}"))
}

async fn read_optional_json(request: Request<Incoming>) -> Result<Value, String> {
    let body = Limited::new(request.into_body(), 1024 * 1024);
    let bytes = body
        .collect()
        .await
        .map_err(|error| format!("读取请求体失败: {error}"))?
        .to_bytes();
    if bytes.is_empty() {
        Ok(json!({}))
    } else {
        serde_json::from_slice(&bytes).map_err(|error| format!("解析 JSON 失败: {error}"))
    }
}

fn sse_frame(event: &str, data: &str) -> Bytes {
    // serde_json 的字符串不会包含原始换行；任务事件同样以 JSON 编码后发送。
    Bytes::from(format!("event: {event}\ndata: {data}\n\n"))
}

fn event_stream(runtime: Arc<ServerRuntime>) -> Response<HttpBody> {
    let mut events = runtime.events.subscribe();
    let (sender, receiver) = mpsc::channel::<Bytes>(64);

    // 先订阅，再发快照。两者之间发生的更新会作为增量事件补发。
    // 如果消费者落后导致广播队列溢出，则再发一份快照恢复一致性。
    tokio::spawn(async move {
        let snapshot = json!(runtime.tasks.list()).to_string();
        if sender
            .send(sse_frame("task-snapshot", &snapshot))
            .await
            .is_err()
        {
            return;
        }
        let mut heartbeat = tokio::time::interval(Duration::from_secs(20));
        loop {
            tokio::select! {
                event = events.recv() => {
                    let frame = match event {
                        Ok(event) => sse_frame(event.name, &event.data),
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                            sse_frame("task-snapshot", &json!(runtime.tasks.list()).to_string())
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    };
                    if sender.send(frame).await.is_err() {
                        break;
                    }
                }
                _ = heartbeat.tick() => {
                    if sender.send(Bytes::from_static(b": heartbeat\n\n")).await.is_err() {
                        break;
                    }
                }
            }
        }
    });

    let frames = stream::unfold(receiver, |mut receiver| async move {
        receiver
            .recv()
            .await
            .map(|bytes| (Ok::<_, Infallible>(Frame::data(bytes)), receiver))
    });
    Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "text/event-stream; charset=utf-8")
        .header("cache-control", "no-cache")
        .header("x-accel-buffering", "no")
        .body(StreamBody::new(frames).boxed_unsync())
        .expect("固定的 SSE 响应头应始终有效")
}

/// 所有任务变更都进入共享 TaskService；HTTP 层只做协议解析和响应编码。
pub async fn handle(
    request: Request<Incoming>,
    runtime: Arc<ServerRuntime>,
) -> Result<Response<HttpBody>, Infallible> {
    let path = request.uri().path().to_string();
    if !path.starts_with("/api/") && path != "/api" {
        return Ok(if request.method() == Method::GET {
            static_response(&runtime.web_dir, &path)
        } else {
            error_response(StatusCode::METHOD_NOT_ALLOWED, "静态资源只支持 GET")
        });
    }
    let supplied_token = request
        .headers()
        .get("authorization")
        .and_then(|header| header.to_str().ok())
        .and_then(|header| header.strip_prefix("Bearer "));
    if !runtime.accepts_token(supplied_token) {
        return Ok(error_response(StatusCode::UNAUTHORIZED, "需要访问令牌"));
    }
    let method = request.method().clone();
    let service = TaskService::new(
        &runtime.tasks,
        &runtime.engine,
        runtime.environment.as_ref(),
    );

    let response = match (method, path.as_str()) {
        (Method::GET, "/api/tasks") => json_response(StatusCode::OK, json!(service.load_tasks())),
        (Method::GET, "/api/events") => event_stream(runtime),
        (Method::GET, "/api/settings") => {
            json_response(StatusCode::OK, runtime.environment.settings())
        }
        (Method::GET, "/api/settings/default-download-dir") => json_response(
            StatusCode::OK,
            json!(runtime.environment.default_download_dir()),
        ),
        (Method::PUT, "/api/settings") => match read_json(request).await {
            Ok(value) => match runtime.save_settings(value) {
                Ok(saved) => json_response(StatusCode::OK, saved),
                Err(error) => error_response(StatusCode::BAD_REQUEST, error),
            },
            Err(error) => error_response(StatusCode::BAD_REQUEST, error),
        },
        (Method::POST, path) if path.starts_with("/api/music/") => {
            let action = path.trim_start_matches("/api/music/");
            match read_json(request).await {
                Ok(value) => match serde_json::from_value::<MusicRequest>(value) {
                    Ok(input) => {
                        match music::execute(action, input, &runtime.environment.artist_separator())
                            .await
                        {
                            Ok(result) => json_response(StatusCode::OK, result),
                            Err(error) => error_response(StatusCode::BAD_REQUEST, error),
                        }
                    }
                    Err(error) => error_response(StatusCode::BAD_REQUEST, error.to_string()),
                },
                Err(error) => error_response(StatusCode::BAD_REQUEST, error),
            }
        }
        (Method::GET, "/api/login/status") => {
            let result = qq_login::get_login_status(runtime.login_store.as_ref()).await;
            legacy_json_result(result)
        }
        (Method::POST, "/api/login/qr") => {
            let store: Arc<dyn LoginCredentialStore> = runtime.login_store.clone();
            legacy_json_result(qq_login::create_qr_login(store).await)
        }
        (Method::POST, "/api/login/logout") => {
            match qq_login::logout(runtime.login_store.as_ref()).await {
                Ok(()) => json_response(StatusCode::OK, json!({ "ok": true })),
                Err(error) => error_response(StatusCode::BAD_REQUEST, error),
            }
        }
        (Method::POST, "/api/login/manual") => match read_json(request).await {
            Ok(value) => match serde_json::from_value::<ManualLoginRequest>(value) {
                Ok(input) => legacy_json_result(
                    qq_login::login_with_uin_authst(
                        runtime.login_store.as_ref(),
                        input.uin,
                        input.authst,
                        input.refresh_token,
                        input.refresh_key,
                        input.access_token,
                        input.openid,
                    )
                    .await,
                ),
                Err(error) => error_response(StatusCode::BAD_REQUEST, error.to_string()),
            },
            Err(error) => error_response(StatusCode::BAD_REQUEST, error),
        },
        (Method::GET, path) if path.starts_with("/api/login/qr/") => {
            let id = path.trim_start_matches("/api/login/qr/");
            if id.is_empty() || id.contains('/') {
                error_response(StatusCode::NOT_FOUND, "未知二维码会话")
            } else {
                legacy_json_result(qq_login::check_qr_login(id.to_string()).await)
            }
        }
        (Method::POST, "/api/tasks") => match read_json(request).await {
            Ok(value) => match serde_json::from_value::<CreateTaskRequest>(value) {
                Ok(input) => match service.create_download_task(input).await {
                    Ok(result) => json_response(StatusCode::OK, json!(result)),
                    Err(error) => error_response(StatusCode::BAD_REQUEST, error),
                },
                Err(error) => error_response(StatusCode::BAD_REQUEST, error.to_string()),
            },
            Err(error) => error_response(StatusCode::BAD_REQUEST, error),
        },
        (Method::POST, "/api/tasks/remove") => match read_json(request).await {
            Ok(value) => match serde_json::from_value::<BatchRemoveRequest>(value) {
                Ok(input) => match service
                    .remove_tasks(input.task_ids, input.delete_file)
                    .await
                {
                    Ok(result) => json_response(StatusCode::OK, json!(result)),
                    Err(error) => error_response(StatusCode::BAD_REQUEST, error),
                },
                Err(error) => error_response(StatusCode::BAD_REQUEST, error.to_string()),
            },
            Err(error) => error_response(StatusCode::BAD_REQUEST, error),
        },
        (Method::POST, path) if path.starts_with("/api/tasks/") => {
            let parts: Vec<_> = path.trim_start_matches("/api/tasks/").split('/').collect();
            if parts.len() != 2 || parts[0].is_empty() {
                error_response(StatusCode::NOT_FOUND, "未知任务操作")
            } else {
                let id = parts[0].to_string();
                let action = parts[1];
                let result = match action {
                    "pause" => service.pause_task(id).await.map(|_| json!({ "ok": true })),
                    "resume" => service.resume_task(id).await.map(|_| json!({ "ok": true })),
                    "retry" => service
                        .retry_task(id)
                        .await
                        .map(|retried| json!({ "retried": retried })),
                    "cancel" | "remove" => {
                        let options = read_optional_json(request).await.and_then(|body| {
                            serde_json::from_value::<DeleteRequest>(body)
                                .map_err(|error| error.to_string())
                        });
                        match options {
                            Ok(options) if action == "cancel" => service
                                .cancel_task(id, options.delete_file)
                                .await
                                .map(|_| json!({ "ok": true })),
                            Ok(options) => service
                                .remove_task(id, options.delete_file)
                                .await
                                .map(|_| json!({ "ok": true })),
                            Err(error) => Err(error),
                        }
                    }
                    _ => Err("未知任务操作".into()),
                };
                match result {
                    Ok(value) => json_response(StatusCode::OK, value),
                    Err(error) => error_response(StatusCode::BAD_REQUEST, error),
                }
            }
        }
        _ => error_response(StatusCode::NOT_FOUND, "未知 API 路径"),
    };
    Ok(response)
}

/// 共享登录接口沿用现有客户端的 JSON 字符串返回值；HTTP 层将其变成 JSON 响应体。
fn legacy_json_result(result: Result<String, String>) -> Response<HttpBody> {
    match result {
        Ok(value) => match serde_json::from_str(&value) {
            Ok(value) => json_response(StatusCode::OK, value),
            Err(error) => error_response(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
        },
        Err(error) => error_response(StatusCode::BAD_REQUEST, error),
    }
}
