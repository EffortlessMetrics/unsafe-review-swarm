use std::collections::{BTreeMap, BTreeSet};

use tower_lsp_server::ls_types::Uri;

#[derive(Default)]
pub(super) struct DocumentStore {
    pub(super) docs: BTreeMap<Uri, String>,
}

pub(super) fn clear_uris_for_failure(previous: &mut BTreeSet<Uri>) -> Vec<Uri> {
    let clear_uris = previous.iter().cloned().collect::<Vec<_>>();
    previous.clear();
    clear_uris
}
