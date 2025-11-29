use buckyos_backup_lib::{BackupResult, BuckyBackupError};
use log::warn;
use std::path::{Path, PathBuf};
use tokio::task;
use url::Url;
use walkdir::WalkDir;

pub async fn create_directory_snapshot(src: &Path, dst: &Path) -> BackupResult<()> {
    let source = src.to_path_buf();
    let target = dst.to_path_buf();

    task::spawn_blocking(move || -> Result<(), std::io::Error> {
        copy_dir_blocking(&source, &target)
    })
    .await
    .map_err(|err| BuckyBackupError::Failed(format!("snapshot worker join error: {}", err)))??;

    Ok(())
}

pub async fn remove_snapshot_dir(path: &Path) -> BackupResult<()> {
    let path_buf = path.to_path_buf();
    task::spawn_blocking(move || -> Result<(), std::io::Error> {
        if !path_buf.exists() {
            return Ok(());
        }
        std::fs::remove_dir_all(&path_buf)
    })
    .await
    .map_err(|err| BuckyBackupError::Failed(format!("snapshot remove join error: {}", err)))??;
    Ok(())
}

pub fn path_to_file_url(path: &Path) -> BackupResult<String> {
    Url::from_file_path(path)
        .map_err(|_| {
            BuckyBackupError::Failed(format!(
                "failed to translate snapshot path {} to file url",
                path.display()
            ))
        })
        .map(|url| url.into_string())
}

fn copy_dir_blocking(src: &Path, dst: &Path) -> Result<(), std::io::Error> {
    if !src.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("source path {} not found", src.display()),
        ));
    }

    if dst.exists() {
        std::fs::remove_dir_all(dst)?;
    }
    std::fs::create_dir_all(dst)?;

    for entry in WalkDir::new(src).follow_links(false) {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    err.to_string(),
                ))
            }
        };
        let rel_path = match entry.path().strip_prefix(src) {
            Ok(rel) => rel,
            Err(_) => continue,
        };

        if rel_path.as_os_str().is_empty() {
            continue;
        }

        let target = dst.join(rel_path);
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&target)?;
            continue;
        }

        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }

        if entry.file_type().is_symlink() {
            match std::fs::metadata(entry.path()) {
                Ok(meta) => {
                    if meta.is_dir() {
                        warn!(
                            "skip directory symlink when creating snapshot: {}",
                            entry.path().display()
                        );
                        continue;
                    }
                }
                Err(err) => {
                    warn!(
                        "skip unreadable symlink {}: {}",
                        entry.path().display(),
                        err
                    );
                    continue;
                }
            }
        }

        std::fs::copy(entry.path(), &target)?;
    }

    Ok(())
}
