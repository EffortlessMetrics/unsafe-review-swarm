use std::path::{Path, PathBuf};

use tower_lsp_server::ls_types::Uri;

pub(super) fn uri_from_path(path: impl AsRef<Path>) -> Option<Uri> {
    Uri::from_file_path(path)
}

pub(super) fn uri_to_path(uri: &Uri) -> Option<PathBuf> {
    uri.to_file_path().map(|path| path.to_path_buf())
}
