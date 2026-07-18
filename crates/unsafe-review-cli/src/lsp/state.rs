use std::collections::{BTreeMap, BTreeSet};

use tower_lsp_server::ls_types::Uri;

#[derive(Default)]
pub(super) struct DocumentStore {
    pub(super) docs: BTreeMap<Uri, DocumentState>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct DocumentState {
    pub(super) text: String,
    pub(super) version: i32,
}

impl DocumentStore {
    pub(super) fn upsert(&mut self, uri: Uri, text: String, version: i32) {
        self.docs.insert(uri, DocumentState { text, version });
    }

    pub(super) fn update_version(&mut self, uri: &Uri, version: i32) {
        if let Some(document) = self.docs.get_mut(uri) {
            document.version = version;
        }
    }

    pub(super) fn remove(&mut self, uri: &Uri) {
        self.docs.remove(uri);
    }

    pub(super) fn version(&self, uri: &Uri) -> Option<i32> {
        self.docs.get(uri).map(|document| document.version)
    }
}

pub(super) fn clear_uris_for_failure(previous: &mut BTreeSet<Uri>) -> Vec<Uri> {
    let clear_uris = previous.iter().cloned().collect::<Vec<_>>();
    previous.clear();
    clear_uris
}

#[cfg(test)]
mod tests {
    use super::DocumentStore;
    use tower_lsp_server::ls_types::Uri;

    #[test]
    fn document_versions_follow_open_change_and_save_updates()
    -> Result<(), Box<dyn std::error::Error>> {
        let uri: Uri = "file:///workspace/src/lib.rs".parse()?;
        let mut store = DocumentStore::default();

        store.upsert(uri.clone(), "fn main() {}".to_string(), 7);
        assert_eq!(store.version(&uri), Some(7));

        store.update_version(&uri, 8);
        assert_eq!(store.version(&uri), Some(8));

        store.remove(&uri);
        assert_eq!(store.version(&uri), None);
        Ok(())
    }

    #[test]
    fn saving_an_unopened_document_does_not_fabricate_a_version()
    -> Result<(), Box<dyn std::error::Error>> {
        let uri: Uri = "file:///workspace/src/lib.rs".parse()?;
        let mut store = DocumentStore::default();

        store.update_version(&uri, 3);
        assert_eq!(store.version(&uri), None);
        Ok(())
    }
}
