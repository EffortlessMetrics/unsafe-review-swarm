use std::path::PathBuf;

use tower_lsp_server::ls_types::{
    CodeActionKind, CodeActionOptions, CodeActionProviderCapability, ExecuteCommandOptions,
    InitializeParams, ServerCapabilities, TextDocumentSyncCapability, TextDocumentSyncKind,
};

use super::uri::uri_to_path;
use super::{CMD_OPEN_TEST, CMD_PACKET, CMD_REFRESH, CMD_WITNESS_COMMAND, CMD_WITNESS_ROUTE};

pub(super) fn server_capabilities() -> ServerCapabilities {
    ServerCapabilities {
        text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
        hover_provider: Some(tower_lsp_server::ls_types::HoverProviderCapability::Simple(
            true,
        )),
        code_action_provider: Some(CodeActionProviderCapability::Options(CodeActionOptions {
            code_action_kinds: Some(
                [
                    "quickfix.unsafeReview.agentPacket",
                    "source.unsafeReview.reviewContext",
                    "source.unsafeReview.witnessRoute",
                    "source.unsafeReview.witnessCommand",
                    "source.unsafeReview.relatedTest",
                ]
                .into_iter()
                .map(|kind| CodeActionKind::from(kind.to_string()))
                .collect(),
            ),
            resolve_provider: Some(false),
            work_done_progress_options: Default::default(),
        })),
        execute_command_provider: Some(ExecuteCommandOptions {
            commands: vec![
                CMD_REFRESH.into(),
                CMD_PACKET.into(),
                CMD_WITNESS_ROUTE.into(),
                CMD_WITNESS_COMMAND.into(),
                CMD_OPEN_TEST.into(),
            ],
            work_done_progress_options: Default::default(),
        }),
        ..Default::default()
    }
}

pub(super) fn root_from_initialize_params(params: &InitializeParams) -> Option<PathBuf> {
    if let Some(folder) = params
        .workspace_folders
        .as_ref()
        .and_then(|folders| folders.first())
        && let Some(path) = uri_to_path(&folder.uri)
    {
        return Some(path);
    }
    deprecated_root_uri(params)
}

#[expect(
    deprecated,
    reason = "root_uri remains the fallback for clients without workspaceFolders"
)]
fn deprecated_root_uri(params: &InitializeParams) -> Option<PathBuf> {
    params.root_uri.as_ref().and_then(uri_to_path)
}
