use std::path::{Path, PathBuf};

use tower_lsp_server::ls_types::Uri;

pub(super) fn uri_from_path(path: impl AsRef<Path>) -> Option<Uri> {
    let path = path.as_ref();
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().ok()?.join(path)
    };
    Uri::from_file_path(path)
}

pub(super) fn uri_to_path(uri: &Uri) -> Option<PathBuf> {
    uri.to_file_path().map(|path| path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::{uri_from_path, uri_to_path};
    use tower_lsp_server::ls_types::Uri;

    #[test]
    fn uri_from_relative_path_round_trips_to_absolute_path() -> Result<(), String> {
        let relative = std::path::Path::new("Cargo.toml");
        let uri = uri_from_path(relative)
            .ok_or_else(|| "relative path should serialize to uri".to_string())?;
        let round_trip =
            uri_to_path(&uri).ok_or_else(|| "uri should convert back to file path".to_string())?;
        assert!(round_trip.is_absolute());
        assert!(round_trip.ends_with(relative));
        Ok(())
    }

    #[test]
    fn uri_from_absolute_path_round_trips_without_rebasing() -> Result<(), String> {
        let absolute = std::env::current_dir()
            .map_err(|err| format!("current_dir failed: {err}"))?
            .join("Cargo.toml");

        let uri = uri_from_path(&absolute)
            .ok_or_else(|| "absolute path should serialize to uri".to_string())?;
        let round_trip =
            uri_to_path(&uri).ok_or_else(|| "uri should convert back to file path".to_string())?;

        assert_eq!(round_trip, absolute);
        Ok(())
    }

    #[test]
    fn uri_to_path_parses_file_uri() -> Result<(), String> {
        let uri = "file:///workspace/unsafe-review-swarm/Cargo.toml"
            .parse::<Uri>()
            .map_err(|err| format!("failed to parse uri: {err}"))?;

        let path = uri_to_path(&uri).ok_or_else(|| "file uri should parse to path".to_string())?;
        assert!(path.ends_with("Cargo.toml"));
        Ok(())
    }
}
