use std::{
    fs,
    io::{BufRead, BufReader, Write},
    os::unix::{
        fs::PermissionsExt,
        net::{UnixListener, UnixStream},
    },
    path::{Path, PathBuf},
    sync::Arc,
    thread,
    time::Duration,
};

use serde::{Deserialize, Serialize};

use crate::{app_launch::LaunchResult, search_result::SearchResult};

const SOCKET_FILE_NAME: &str = "rat-search.sock";
const FALLBACK_SOCKET_PREFIX: &str = "rat-search";
const SOCKET_RESPONSE_TIMEOUT_MS: u64 = 1_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum IpcRequest {
    Health,
    Search {
        query: String,
        limit: usize,
    },
    RecordSearchHistory {
        query: String,
    },
    LaunchApp {
        app_id: String,
    },
    OpenPath {
        path: String,
    },
    OpenInCode {
        path: String,
    },
    RevealPath {
        path: String,
    },
    CopyPath {
        path: String,
    },
    CopyText {
        text: String,
    },
    OpenCalculatorApp {
        expression: String,
        result: String,
        copy_text: String,
    },
    OpenUrl {
        url: String,
    },
    OpenSetting {
        setting_id: String,
    },
    CopyClipboardItem {
        item_id: String,
    },
    DeleteClipboardItem {
        item_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum IpcResponse {
    Health { ready: bool },
    Search { results: Vec<SearchResult> },
    LaunchApp { result: LaunchResult },
    Unit,
    Error { message: String },
}

pub(crate) fn socket_path() -> PathBuf {
    match std::env::var_os("XDG_RUNTIME_DIR") {
        Some(runtime_dir) if !runtime_dir.is_empty() => {
            PathBuf::from(runtime_dir).join(SOCKET_FILE_NAME)
        }
        _ => std::env::temp_dir().join(format!(
            "{FALLBACK_SOCKET_PREFIX}-{}.sock",
            current_uid_label()
        )),
    }
}

fn current_uid_label() -> String {
    std::env::var("UID")
        .ok()
        .filter(|uid| !uid.trim().is_empty())
        .unwrap_or_else(|| "user".to_owned())
}

pub(crate) fn cleanup_stale_socket(path: &Path) -> Result<bool, String> {
    if !path.exists() {
        return Ok(false);
    }

    if UnixStream::connect(path).is_ok() {
        return Err("Rat Search IPC socket is already active".to_owned());
    }

    fs::remove_file(path).map_err(|error| {
        format!(
            "failed to remove stale IPC socket '{}': {error}",
            path.display()
        )
    })?;

    Ok(true)
}

pub(crate) fn start_server<F>(handler: F) -> Result<PathBuf, String>
where
    F: Fn(IpcRequest) -> IpcResponse + Send + Sync + 'static,
{
    let path = socket_path();

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create IPC socket directory '{}': {error}",
                parent.display()
            )
        })?;
    }

    cleanup_stale_socket(&path)?;
    let listener = UnixListener::bind(&path)
        .map_err(|error| format!("failed to bind IPC socket '{}': {error}", path.display()))?;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).map_err(|error| {
        format!(
            "failed to set IPC socket permissions '{}': {error}",
            path.display()
        )
    })?;

    let handler = Arc::new(handler);
    thread::spawn(move || {
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let handler = handler.clone();
                    thread::spawn(move || handle_stream(stream, handler));
                }
                Err(error) => eprintln!("failed to accept IPC connection: {error}"),
            }
        }
    });

    Ok(path)
}

fn handle_stream<F>(stream: UnixStream, handler: Arc<F>)
where
    F: Fn(IpcRequest) -> IpcResponse + Send + Sync + 'static,
{
    let mut writer = match stream.try_clone() {
        Ok(writer) => writer,
        Err(error) => {
            eprintln!("failed to clone IPC stream: {error}");
            return;
        }
    };
    let mut reader = BufReader::new(stream);
    let mut line = String::new();

    match reader.read_line(&mut line) {
        Ok(0) => return,
        Ok(_) => {}
        Err(error) => {
            eprintln!("failed to read IPC request: {error}");
            return;
        }
    }

    let response = match serde_json::from_str::<IpcRequest>(&line) {
        Ok(request) => handler(request),
        Err(_) => IpcResponse::Error {
            message: "Malformed IPC request".to_owned(),
        },
    };

    if let Err(error) = write_response(&mut writer, &response) {
        eprintln!("failed to write IPC response: {error}");
    }
}

pub(crate) fn send_request(request: &IpcRequest) -> Result<IpcResponse, String> {
    send_request_to_path(&socket_path(), request)
}

pub(crate) fn send_request_to_path(
    path: &Path,
    request: &IpcRequest,
) -> Result<IpcResponse, String> {
    let stream = UnixStream::connect(path)
        .map_err(|error| format!("failed to connect IPC socket '{}': {error}", path.display()))?;
    stream
        .set_read_timeout(Some(Duration::from_millis(SOCKET_RESPONSE_TIMEOUT_MS)))
        .map_err(|error| format!("failed to set IPC read timeout: {error}"))?;
    stream
        .set_write_timeout(Some(Duration::from_millis(SOCKET_RESPONSE_TIMEOUT_MS)))
        .map_err(|error| format!("failed to set IPC write timeout: {error}"))?;

    let mut writer = stream
        .try_clone()
        .map_err(|error| format!("failed to clone IPC stream: {error}"))?;
    let mut reader = BufReader::new(stream);
    let request = serde_json::to_string(request)
        .map_err(|error| format!("failed to serialize IPC request: {error}"))?;

    writer
        .write_all(request.as_bytes())
        .and_then(|_| writer.write_all(b"\n"))
        .and_then(|_| writer.flush())
        .map_err(|error| format!("failed to write IPC request: {error}"))?;

    let mut line = String::new();
    reader
        .read_line(&mut line)
        .map_err(|error| format!("failed to read IPC response: {error}"))?;

    serde_json::from_str::<IpcResponse>(&line)
        .map_err(|error| format!("failed to parse IPC response: {error}"))
}

fn write_response(writer: &mut UnixStream, response: &IpcResponse) -> Result<(), String> {
    let response = serde_json::to_string(response)
        .map_err(|error| format!("failed to serialize IPC response: {error}"))?;

    writer
        .write_all(response.as_bytes())
        .and_then(|_| writer.write_all(b"\n"))
        .and_then(|_| writer.flush())
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ipc_request_round_trip_serializes_tagged_payload() {
        let request = IpcRequest::Search {
            query: "fire".to_owned(),
            limit: 8,
        };
        let value = serde_json::to_string(&request).expect("request should serialize");

        assert_eq!(
            serde_json::from_str::<IpcRequest>(&value).expect("request should deserialize"),
            request
        );
    }

    #[test]
    fn ipc_response_round_trip_serializes_error() {
        let response = IpcResponse::Error {
            message: "Nope".to_owned(),
        };
        let value = serde_json::to_string(&response).expect("response should serialize");

        assert_eq!(
            serde_json::from_str::<IpcResponse>(&value).expect("response should deserialize"),
            response
        );
    }

    #[test]
    fn fallback_socket_path_uses_user_specific_name_without_runtime_dir() {
        let previous_runtime_dir = std::env::var_os("XDG_RUNTIME_DIR");
        std::env::remove_var("XDG_RUNTIME_DIR");

        let path = socket_path();

        match previous_runtime_dir {
            Some(value) => std::env::set_var("XDG_RUNTIME_DIR", value),
            None => std::env::remove_var("XDG_RUNTIME_DIR"),
        }

        assert!(path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("rat-search-") && name.ends_with(".sock")));
    }

    #[test]
    fn stale_socket_cleanup_removes_non_socket_file() {
        let path = std::env::temp_dir().join(format!("rat-search-ipc-test-{}", std::process::id()));
        fs::write(&path, "stale").expect("stale file should be written");

        assert_eq!(cleanup_stale_socket(&path), Ok(true));
        assert!(!path.exists());
    }
}
