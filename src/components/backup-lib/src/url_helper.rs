use url::Url;

use crate::{BackupResult, BuckyBackupError};

pub fn translate_local_path_from_url(url: &str) -> BackupResult<String> {
    if url.starts_with("file://") {
        let url = Url::parse(&url).map_err(|e| BuckyBackupError::Failed(e.to_string()))?;
        Ok(url.path().to_string())
    } else {
        Ok(url.to_string())
    }
}
