use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

const INITIALIZE_ID: u64 = 1;
const SHUTDOWN_ID: u64 = 2;
const HOVER_ID: u64 = 3;
const CODE_ACTION_ID: u64 = 4;
const PACKET_ID: u64 = 5;
const WITNESS_ROUTE_ID: u64 = 6;
const WITNESS_COMMAND_ID: u64 = 7;
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
    let (card_id, position) = validate_diagnostics_notification(&diagnostics)?;
    let source_uri = file_uri(&fixture_root.join("src/lib.rs"));

    write_message(
        &mut stdin,
        &json!({
            "jsonrpc": "2.0",
            "id": HOVER_ID,
            "method": "textDocument/hover",
            "params": {
                "textDocument": {"uri": source_uri.clone()},
                "position": position.clone()
            }
        }),
    )?;
    let hover = wait_for_id(&messages_rx, HOVER_ID)?;
    validate_hover_response(&hover, &card_id)?;

    write_message(
        &mut stdin,
        &json!({
            "jsonrpc": "2.0",
            "id": CODE_ACTION_ID,
            "method": "textDocument/codeAction",
            "params": {
                "textDocument": {"uri": source_uri},
                "range": {"start": position.clone(), "end": position},
                "context": {"diagnostics": []}
            }
        }),
    )?;
    let code_actions = wait_for_id(&messages_rx, CODE_ACTION_ID)?;
    validate_code_actions_response(&code_actions, &card_id)?;

    write_message(
        &mut stdin,
        &json!({
            "jsonrpc": "2.0",
            "id": PACKET_ID,
            "method": "workspace/executeCommand",
            "params": {
                "command": "unsafe-review.collectAgentPacket",
                "arguments": [{"card_id": card_id}]
            }
        }),
    )?;
    let packet = wait_for_id(&messages_rx, PACKET_ID)?;
    validate_packet_response(&packet, &card_id)?;

    write_message(
        &mut stdin,
        &json!({
            "jsonrpc": "2.0",
            "id": WITNESS_ROUTE_ID,
            "method": "workspace/executeCommand",
            "params": {
                "command": "unsafe-review.explainWitnessRoute",
                "arguments": [{"card_id": card_id}]
            }
        }),
    )?;
    let witness_route = wait_for_id(&messages_rx, WITNESS_ROUTE_ID)?;
    validate_witness_route_response(&witness_route, &card_id)?;

    write_message(
        &mut stdin,
        &json!({
            "jsonrpc": "2.0",
            "id": WITNESS_COMMAND_ID,
            "method": "workspace/executeCommand",
            "params": {
                "command": "unsafe-review.collectWitnessCommand",
                "arguments": [{"card_id": card_id}]
            }
        }),
    )?;
    let witness_command = wait_for_id(&messages_rx, WITNESS_COMMAND_ID)?;
    validate_witness_command_response(&witness_command, &card_id)?;

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

fn validate_diagnostics_notification(notification: &Value) -> Result<(String, Value), String> {
    let diagnostics = notification["params"]["diagnostics"]
        .as_array()
        .ok_or_else(|| "publishDiagnostics has no diagnostics array".to_string())?;
    let diagnostic = diagnostics
        .first()
        .ok_or_else(|| "LSP smoke expected a diagnostic from raw_pointer_alignment".to_string())?;
    let card_id = diagnostic["data"]["card_id"]
        .as_str()
        .ok_or_else(|| "published diagnostic has no canonical card_id".to_string())?
        .to_owned();
    let position = diagnostic["range"]["start"].clone();
    if diagnostic["source"] != "unsafe-review"
        || diagnostic["data"]["operation_family"].as_str().is_none()
        || diagnostic["data"]["coverage"].is_null()
        || position["line"].as_u64().is_none()
        || position["character"].as_u64().is_none()
    {
        return Err(format!(
            "published diagnostic lacks canonical ReviewCard data: {diagnostic}"
        ));
    }
    Ok((card_id, position))
}

fn validate_hover_response(response: &Value, card_id: &str) -> Result<(), String> {
    let contents = response["result"]["contents"]["value"]
        .as_str()
        .ok_or_else(|| format!("hover response has no markdown contents: {response}"))?;
    if !contents.contains(card_id) || !contents.contains("not memory-safety proof") {
        return Err(format!(
            "hover response lost canonical identity or trust boundary: {response}"
        ));
    }
    Ok(())
}

fn validate_code_actions_response(response: &Value, card_id: &str) -> Result<(), String> {
    let actions = response["result"]
        .as_array()
        .ok_or_else(|| format!("code-action response has no action array: {response}"))?;
    if actions.is_empty()
        || actions.iter().any(|action| action.get("edit").is_some())
        || !actions.iter().any(|action| {
            action["command"] == "unsafe-review.collectAgentPacket"
                && action["arguments"][0]["card_id"] == card_id
        })
    {
        return Err(format!(
            "code-action response is not command-only or lost card identity: {response}"
        ));
    }
    Ok(())
}

fn validate_packet_response(response: &Value, card_id: &str) -> Result<(), String> {
    let packet_text = response["result"]
        .as_str()
        .ok_or_else(|| format!("agent-packet command returned no packet: {response}"))?;
    let packet: Value = serde_json::from_str(packet_text)
        .map_err(|error| format!("agent-packet command returned invalid JSON: {error}"))?;
    if packet["card_id"] != card_id
        || packet["repair_scope"] != "this card only"
        || !packet["confirmation_cue"].is_object()
        || !packet["do_not_do"].is_array()
        || !packet["trust_boundary"]
            .as_str()
            .is_some_and(|boundary| boundary.contains("not UB-free status"))
    {
        return Err(format!(
            "agent packet is incomplete or unbounded for {card_id}: {packet}"
        ));
    }
    Ok(())
}

fn validate_witness_route_response(response: &Value, card_id: &str) -> Result<(), String> {
    if response["result"]["kind"] != "unsafe-review.witness_route"
        || response["result"]["card_id"] != card_id
        || response["result"]["route"].as_str().is_none()
        || !response["result"]["trust_boundary"]
            .as_str()
            .is_some_and(|boundary| boundary.contains("not a site-execution claim"))
    {
        return Err(format!(
            "witness-route response lost bounded receipt guidance: {response}"
        ));
    }
    Ok(())
}

fn validate_witness_command_response(response: &Value, card_id: &str) -> Result<(), String> {
    if response["result"]["kind"] != "unsafe-review.witness_command"
        || response["result"]["card_id"] != card_id
        || response["result"]["command"].as_str().is_none()
        || !response["result"]["trust_boundary"]
            .as_str()
            .is_some_and(|boundary| boundary.contains("not a site-execution claim"))
    {
        return Err(format!(
            "witness-command response lost verification boundary: {response}"
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
