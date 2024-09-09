use crate::{checkpoint::StorageReader, error::BackupResult, meta::CheckPointMeta};

pub async fn meta_from_reader(
    reader: &dyn StorageReader,
) -> BackupResult<CheckPointMeta<String, String, String, String, String>> {
    unimplemented!()
}

// meta = meta(target_reader) - meta(source_reader)
pub async fn meta_from_delta(
    source_reader: &dyn StorageReader,
    target_reader: &dyn StorageReader,
) -> BackupResult<CheckPointMeta<String, String, String, String, String>> {
    unimplemented!()
}
