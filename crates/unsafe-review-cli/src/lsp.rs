mod actions;
mod backend;
mod capabilities;
mod config;
mod diagnostics;
mod hover;
mod state;
#[cfg(test)]
mod tests;
mod uri;

use backend::Backend;
use tower_lsp_server::{LspService, Server};

pub(super) const CMD_REFRESH: &str = "unsafe-review.refresh";
pub(super) const CMD_PACKET: &str = "unsafe-review.collectAgentPacket";
pub(super) const CMD_WITNESS_ROUTE: &str = "unsafe-review.explainWitnessRoute";
pub(super) const CMD_WITNESS_COMMAND: &str = "unsafe-review.collectWitnessCommand";
pub(super) const CMD_OPEN_TEST: &str = "unsafe-review.openRelatedTest";
pub(super) const TRUST_BOUNDARY: &str = "Static unsafe-contract review only. This is not memory-safety proof, not UB-free status, and not a Miri result unless a matching witness receipt is attached.";

pub(crate) fn serve() -> Result<(), String> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("tokio runtime init failed: {e}"))?;
    runtime.block_on(async {
        let stdin = tokio::io::stdin();
        let stdout = tokio::io::stdout();
        let (service, socket) = LspService::new(Backend::new);
        Server::new(stdin, stdout, socket).serve(service).await;
    });
    Ok(())
}
