use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

const INITIALIZE_ID: u64 = 1;
const SHUTDOWN_ID: u64 = 2;
const MESSAGE_TIMEOUT: Duration = Duration::from_secs(20);

pub(crate) fn run(workspace_root: &Path) -> Result<(), String> {
    let fixture_root = workspace_root.join("fixtures/raw_pointer_alignment");
    if !fixture_root.is_dir() {
        return Err(format!(
            "LSP smoke fixture is missing: {}",
            fixture_root.display()
        ));
    }

    let mut child = Command::new("cargo")
        .args([
            "run",
            "--locked",
            "-p",
            "unsafe-review-cli",
            "--bin",
            "cargo-unsafe-review",
            "--",
            "lsp",
        ])
        .current_dir(workspace_root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("failed to start live LSP server: {error}"))?;

    let result = protocol_smoke(&mut child, &fixture_root);
    if result.is_err() {
        let _ = child.kill();
    }
    let status = child
        .wait()
        .map_err(|error| format!("failed waiting for live LSP server: {error}"))?;
    result.and_then(|()| {
        if status.success() {
            println!("lsp-smoke: ok");
            Ok(())
        } else {
            Err(format!("live LSP server exited with status {status}"))
        }
    })
}

fn protocol_smoke(child: &mut Child, fixture_root: &Path) -> Result<(), String> {
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| "live LSP server stdin was not piped".to_string())?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "live LSP server stdout was not piped".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "live LSP server stderr was not piped".to_string())?;

    // Drain stderr while the protocol runs so cargo diagnostics cannot block
    // the child. The smoke result is based on the JSON-RPC stream, not logs.
    thread::spawn(move || {
        let mut reader = BufReader::new(stderr);
        let mut discarded = String::new();
        let _ = reader.read_to_string(&mut discarded);
    });

    let (messages_tx, messages_rx) = mpsc::channel();
    thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        loop {
            let message = read_message(&mut reader);
            let done = message.is_err();
            if messages_tx.send(message).is_err() || done {
                break;
            }
        }
    });

    write_message(
        &mut stdin,
        &json!({
            "jsonrpc": "2.0",
            "id": INITIALIZE_ID,
            "method": "initialize",
            "params": {
                "processId": null,
                "rootUri": file_uri(fixture_root),
                "capabilities": {},
                "workspaceFolders": [{
                    "uri": file_uri(fixture_root),
                    "name": "raw_pointer_alignment"
                }]
            }
        }),
    )?;
    let initialize = wait_for_id(&messages_rx, INITIALIZE_ID)?;
    validate_initialize_response(&initialize)?;

    write_message(
        &mut stdin,
        &json!({
            "jsonrpc": "2.0",
            "method": "initialized",
            "params": {}
        }),
    )?;
    let diagnostics = wait_for_method(&messages_rx, "textDocument/publishDiagnostics")?;
    validate_diagnostics_notification(&diagnostics)?;

    write_message(
        &mut stdin,
        &json!({
            "jsonrpc": "2.0",
            "id": SHUTDOWN_ID,
            "method": "shutdown",
            "params": null
        }),
    )?;
    let shutdown = wait_for_id(&messages_rx, SHUTDOWN_ID)?;
    if shutdown.get("error").is_some() {
        return Err(format!("live LSP shutdown returned an error: {shutdown}"));
    }
    write_message(
        &mut stdin,
        &json!({
            "jsonrpc": "2.0",
            "method": "exit",
            "params": null
        }),
    )?;
    Ok(())
}

fn validate_initialize_response(response: &Value) -> Result<(), String> {
    let capabilities = response
        .get("result")
        .and_then(|result| result.get("capabilities"))
        .ok_or_else(|| format!("initialize response has no capabilities: {response}"))?;
    if capabilities["textDocumentSync"].is_null()
        || capabilities["hoverProvider"] != Value::Bool(true)
        || capabilities["codeActionProvider"] != Value::Bool(true)
    {
        return Err(format!(
            "initialize response lacks required read-only capabilities: {capabilities}"
        ));
    }
    let commands = capabilities["executeCommandProvider"]["commands"]
        .as_array()
        .ok_or_else(|| "initialize response has no execute-command list".to_string())?;
    for required in [
        "unsafe-review.refresh",
        "unsafe-review.collectAgentPacket",
        "unsafe-review.explainWitnessRoute",
        "unsafe-review.collectWitnessCommand",
        "unsafe-review.openRelatedTest",
    ] {
        if !commands.iter().any(|command| command == required) {
            return Err(format!(
                "initialize response is missing execute command `{required}`"
            ));
        }
    }
    Ok(())
}

fn validate_diagnostics_notification(notification: &Value) -> Result<(), String> {
    let diagnostics = notification["params"]["diagnostics"]
        .as_array()
        .ok_or_else(|| "publishDiagnostics has no diagnostics array".to_string())?;
    let diagnostic = diagnostics
        .first()
        .ok_or_else(|| "LSP smoke expected a diagnostic from raw_pointer_alignment".to_string())?;
    if diagnostic["source"] != "unsafe-review"
        || diagnostic["data"]["card_id"].as_str().is_none()
        || diagnostic["data"]["operation_family"].as_str().is_none()
        || diagnostic["data"]["coverage"].is_null()
    {
        return Err(format!(
            "published diagnostic lacks canonical ReviewCard data: {diagnostic}"
        ));
    }
    Ok(())
}

fn wait_for_id(messages: &Receiver<Result<Value, String>>, id: u64) -> Result<Value, String> {
    let deadline = Instant::now() + MESSAGE_TIMEOUT;
    loop {
        let message = receive_until(messages, deadline)?;
        if message.get("id").and_then(Value::as_u64) == Some(id) {
            return Ok(message);
        }
    }
}

fn wait_for_method(
    messages: &Receiver<Result<Value, String>>,
    method: &str,
) -> Result<Value, String> {
    let deadline = Instant::now() + MESSAGE_TIMEOUT;
    loop {
        let message = receive_until(messages, deadline)?;
        if message.get("method").and_then(Value::as_str) == Some(method) {
            return Ok(message);
        }
    }
}

fn receive_until(
    messages: &Receiver<Result<Value, String>>,
    deadline: Instant,
) -> Result<Value, String> {
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return Err("timed out waiting for live LSP message".to_string());
    }
    messages
        .recv_timeout(remaining)
        .map_err(|error| format!("timed out waiting for live LSP message: {error}"))?
}

fn read_message(reader: &mut BufReader<impl Read>) -> Result<Value, String> {
    let mut content_length = None;
    loop {
        let mut line = String::new();
        let read = reader
            .read_line(&mut line)
            .map_err(|error| format!("failed reading LSP header: {error}"))?;
        if read == 0 {
            return Err("live LSP server closed stdout before sending a message".to_string());
        }
        let line = line.trim_end_matches(['\r', '\n']);
        if line.is_empty() {
            break;
        }
        if let Some(value) = line.strip_prefix("Content-Length:") {
            content_length = Some(
                value
                    .trim()
                    .parse::<usize>()
                    .map_err(|error| format!("invalid LSP Content-Length: {error}"))?,
            );
        }
    }
    let length = content_length.ok_or_else(|| "LSP message has no Content-Length".to_string())?;
    let mut body = vec![0; length];
    reader
        .read_exact(&mut body)
        .map_err(|error| format!("failed reading LSP message body: {error}"))?;
    serde_json::from_slice(&body).map_err(|error| format!("invalid LSP JSON message: {error}"))
}

fn write_message(writer: &mut impl Write, message: &Value) -> Result<(), String> {
    let body =
        serde_json::to_vec(message).map_err(|error| format!("encode LSP message: {error}"))?;
    write!(writer, "Content-Length: {}\r\n\r\n", body.len())
        .map_err(|error| format!("write LSP header: {error}"))?;
    writer
        .write_all(&body)
        .map_err(|error| format!("write LSP body: {error}"))?;
    writer
        .flush()
        .map_err(|error| format!("flush LSP message: {error}"))
}

fn file_uri(path: &Path) -> String {
    let raw = path.to_string_lossy().replace('\\', "/");
    let encoded = raw.bytes().fold(String::new(), |mut uri, byte| {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~' | b'/') {
            uri.push(byte as char);
        } else {
            uri.push_str(&format!("%{byte:02X}"));
        }
        uri
    });
    if encoded.starts_with('/') {
        format!("file://{encoded}")
    } else {
        format!("file:///{encoded}")
    }
}

#[cfg(test)]
mod tests {
    use super::file_uri;
    use std::path::Path;

    #[test]
    fn file_uri_is_absolute_and_percent_encoded() {
        assert_eq!(
            file_uri(Path::new("/tmp/review space/src/lib.rs")),
            "file:///tmp/review%20space/src/lib.rs"
        );
    }
}
