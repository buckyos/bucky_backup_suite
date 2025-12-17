use async_trait::async_trait;
use buckyos_backup_lib::{BackupResult, BuckyBackupError};
use buckyos_kit::get_buckyos_service_data_dir;
use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};
use tokio::fs;

const META_SUFFIX: &str = "meta";

#[derive(Debug, Clone)]
pub struct SnapshotInfo {
    pub final_path: PathBuf,
    pub root_path: PathBuf,
}

#[derive(Debug)]
pub struct SnapshotCreateContext<'a> {
    pub plan_id: &'a str,
    pub task_id: &'a str,
    pub source_path: &'a Path,
    pub candidate_root: PathBuf,
}

#[derive(Debug)]
struct BackendSnapshot {
    info: SnapshotInfo,
    metadata: Value,
}

#[async_trait]
trait SnapshotBackend: Sync + Send {
    fn id(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn platform(&self) -> SnapshotPlatform;

    fn is_supported(&self) -> bool {
        true
    }

    async fn create_snapshot(
        &self,
        ctx: SnapshotCreateContext<'_>,
    ) -> BackupResult<BackendSnapshot>;

    async fn cleanup_snapshot(&self, root: &Path, metadata: Value) -> BackupResult<()>;
}

#[derive(Debug, Clone, Copy)]
enum SnapshotPlatform {
    Linux,
    Windows,
}

#[derive(Debug, Serialize, Deserialize)]
struct PersistedSnapshotMetadata {
    backend: String,
    data: Value,
}

impl PersistedSnapshotMetadata {
    fn new(backend: &dyn SnapshotBackend, data: Value) -> Self {
        Self {
            backend: backend.id().to_string(),
            data,
        }
    }
}

pub async fn create_snapshot(
    plan_id: &str,
    task_id: &str,
    source_path: &Path,
) -> BackupResult<SnapshotInfo> {
    let sanitized_plan = sanitize_component(plan_id);
    let sanitized_task = sanitize_component(task_id);
    let mut attempted = false;
    let mut last_err: Option<BuckyBackupError> = None;

    for backend in available_backends().iter().copied() {
        if !backend.is_supported() {
            continue;
        }
        attempted = true;
        let backend_token = backend.id().replace(':', "_");
        let candidate_root = snapshots_root_dir().join(format!(
            "{}_{}_{}_{}",
            sanitized_plan,
            sanitized_task,
            backend_token,
            short_id()
        ));
        let ctx = SnapshotCreateContext {
            plan_id,
            task_id,
            source_path,
            candidate_root,
        };
        match backend.create_snapshot(ctx).await {
            Ok(result) => {
                let persisted = PersistedSnapshotMetadata::new(backend, result.metadata);
                write_metadata(&result.info.root_path, &persisted).await?;
                return Ok(result.info);
            }
            Err(err) => {
                warn!("snapshot backend {} failed: {}", backend.description(), err);
                last_err = Some(err);
            }
        }
    }

    if !attempted {
        return Err(BuckyBackupError::Failed(
            "no snapshot backend available on this platform".to_string(),
        ));
    }

    Err(last_err
        .unwrap_or_else(|| BuckyBackupError::Failed("all snapshot backends failed".to_string())))
}

pub async fn remove_snapshot_dir(snapshot_path: &Path) -> BackupResult<()> {
    let maybe_root = locate_snapshot_root(snapshot_path).await;
    let Some((root_path, metadata)) = maybe_root else {
        debug!(
            "snapshot metadata missing for {}, removing directory directly",
            snapshot_path.display()
        );
        if let Some(inferred_root) = infer_snapshot_root(snapshot_path) {
            return reset_mount_dir(&inferred_root).await;
        } else {
            warn!(
                "skip removing snapshot {}, path is outside snapshot workspace",
                snapshot_path.display()
            );
            return Ok(());
        }
    };

    let backend = backend_by_id(&metadata.backend).ok_or_else(|| {
        BuckyBackupError::Failed(format!(
            "snapshot backend {} unavailable on current platform",
            metadata.backend
        ))
    })?;
    backend
        .cleanup_snapshot(&root_path, metadata.data.clone())
        .await?;

    if let Err(err) = remove_metadata_file(&root_path).await {
        warn!(
            "failed to remove snapshot metadata file for {}: {}",
            root_path.display(),
            err
        );
    }
    Ok(())
}

fn snapshots_root_dir() -> PathBuf {
    get_buckyos_service_data_dir("backup_suite").join("snapshots")
}

fn sanitize_component(raw: &str) -> String {
    raw.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

fn short_id() -> String {
    let id = uuid::Uuid::new_v4().simple().to_string();
    id.chars().take(12).collect()
}

fn metadata_path_for_root(root: &Path) -> PathBuf {
    root.with_extension(META_SUFFIX)
}

fn unescape_findmnt_value(raw: &str) -> String {
    raw.trim_matches('"').replace("\\040", " ")
}

fn infer_snapshot_root(path: &Path) -> Option<PathBuf> {
    let workspace = snapshots_root_dir();
    if !path.starts_with(&workspace) {
        return None;
    }
    let mut current = path.to_path_buf();
    loop {
        if current
            .parent()
            .map(|p| p == workspace.as_path())
            .unwrap_or(false)
        {
            return Some(current);
        }
        if !current.pop() {
            break;
        }
    }
    None
}

async fn remove_metadata_file(root: &Path) -> std::io::Result<()> {
    let meta_path = metadata_path_for_root(root);
    match fs::remove_file(&meta_path).await {
        Ok(_) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err),
    }
}

async fn locate_snapshot_root(path: &Path) -> Option<(PathBuf, PersistedSnapshotMetadata)> {
    let mut current = path.to_path_buf();
    loop {
        let meta_path = metadata_path_for_root(&current);
        match fs::read_to_string(&meta_path).await {
            Ok(content) => match serde_json::from_str::<PersistedSnapshotMetadata>(&content) {
                Ok(metadata) => return Some((current.clone(), metadata)),
                Err(err) => {
                    warn!(
                        "failed to parse snapshot metadata {}: {}",
                        meta_path.display(),
                        err
                    );
                    return None;
                }
            },
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                if !current.pop() {
                    break;
                }
            }
            Err(err) => {
                warn!(
                    "failed to inspect snapshot metadata {}: {}",
                    meta_path.display(),
                    err
                );
                break;
            }
        }
    }
    None
}

async fn reset_mount_dir(path: &Path) -> BackupResult<()> {
    #[cfg(target_os = "windows")]
    {
        match fs::symlink_metadata(path).await {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() || metadata.file_type().is_dir() {
                    match fs::remove_dir(path).await {
                        Ok(_) => Ok(()),
                        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
                        Err(err) => Err(BuckyBackupError::Failed(err.to_string())),
                    }
                } else {
                    Ok(())
                }
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(BuckyBackupError::Failed(err.to_string())),
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        match fs::remove_dir_all(path).await {
            Ok(_) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(BuckyBackupError::Failed(err.to_string())),
        }
    }
}

fn backend_by_id(id: &str) -> Option<&'static dyn SnapshotBackend> {
    available_backends()
        .iter()
        .copied()
        .find(|backend| backend.id() == id && backend.is_supported())
}

fn available_backends() -> &'static [&'static dyn SnapshotBackend] {
    #[cfg(target_os = "linux")]
    {
        static BACKENDS: [&'static dyn SnapshotBackend; 2] =
            [&LINUX_LVM_BACKEND, &LINUX_DM_BACKEND];
        &BACKENDS
    }
    #[cfg(target_os = "windows")]
    {
        static BACKENDS: [&'static dyn SnapshotBackend; 1] = [&WINDOWS_VSS_BACKEND];
        &BACKENDS
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        static BACKENDS: [&'static dyn SnapshotBackend; 0] = [];
        &BACKENDS
    }
}

async fn write_metadata(root: &Path, metadata: &PersistedSnapshotMetadata) -> BackupResult<()> {
    let meta_path = metadata_path_for_root(root);
    let content = serde_json::to_string_pretty(metadata)
        .map_err(|err| BuckyBackupError::Failed(err.to_string()))?;
    if let Some(parent) = meta_path.parent() {
        fs::create_dir_all(parent)
            .await
            .map_err(|err| BuckyBackupError::Failed(err.to_string()))?;
    }
    fs::write(&meta_path, content)
        .await
        .map_err(|err| BuckyBackupError::Failed(err.to_string()))
}

#[cfg(target_os = "linux")]
mod linux {
    use super::*;
    use log::warn;
    use std::path::Path;
    use tokio::process::Command;

    pub(super) struct LinuxLvmBackend;

    #[derive(Debug, Serialize, Deserialize)]
    struct LvmSnapshotMetadata {
        snapshot_lv_path: String,
    }

    #[derive(Debug)]
    struct MountInfo {
        source: String,
        target: PathBuf,
    }

    #[derive(Debug)]
    struct LvmInfo {
        vg_name: String,
        lv_name: String,
        lv_path: String,
    }

    const SNAPSHOT_EXTENTS: &str = "20%ORIGIN";

    #[async_trait]
    impl SnapshotBackend for LinuxLvmBackend {
        fn id(&self) -> &'static str {
            "linux:lvm"
        }

        fn description(&self) -> &'static str {
            "Linux LVM snapshots"
        }

        fn platform(&self) -> SnapshotPlatform {
            SnapshotPlatform::Linux
        }

        async fn create_snapshot(
            &self,
            ctx: SnapshotCreateContext<'_>,
        ) -> BackupResult<BackendSnapshot> {
            let mount_root = ctx.candidate_root;
            let canonical = tokio::fs::canonicalize(ctx.source_path)
                .await
                .map_err(|err| BuckyBackupError::Failed(err.to_string()))?;
            let mount_info = find_mount_point(&canonical).await?;
            let lvm_info = describe_lv(&mount_info.source).await?;

            ensure_clean_dir(&mount_root).await?;
            let relative = canonical
                .strip_prefix(&mount_info.target)
                .unwrap_or(Path::new(""));

            let snapshot_name = format!("bksnap_{}", short_id());
            let snapshot_lv_path = format!("/dev/{}/{}", lvm_info.vg_name, snapshot_name);

            create_lvm_snapshot(&lvm_info.lv_path, &snapshot_name).await?;
            if let Err(err) = mount_snapshot(&snapshot_lv_path, &mount_root).await {
                let _ = remove_snapshot_lv(&snapshot_lv_path).await;
                let _ = reset_mount_dir(&mount_root).await;
                return Err(err);
            }

            let final_path = if relative.as_os_str().is_empty() {
                mount_root.clone()
            } else {
                mount_root.join(relative)
            };

            let metadata = serde_json::to_value(LvmSnapshotMetadata {
                snapshot_lv_path: snapshot_lv_path.clone(),
            })
            .map_err(|err| BuckyBackupError::Failed(err.to_string()))?;

            info!(
                "created LVM snapshot {} for {} mounted at {}",
                snapshot_lv_path,
                ctx.source_path.display(),
                mount_root.display()
            );

            Ok(BackendSnapshot {
                info: SnapshotInfo {
                    final_path,
                    root_path: mount_root,
                },
                metadata,
            })
        }

        async fn cleanup_snapshot(&self, root: &Path, metadata: Value) -> BackupResult<()> {
            let detail: LvmSnapshotMetadata = serde_json::from_value(metadata)
                .map_err(|err| BuckyBackupError::Failed(err.to_string()))?;
            if root.exists() {
                if let Err(err) = run_command("umount", &[root.to_string_lossy().as_ref()]).await {
                    warn!("failed to unmount snapshot at {}: {}", root.display(), err);
                }
            }
            if let Err(err) = remove_snapshot_lv(&detail.snapshot_lv_path).await {
                warn!(
                    "failed to remove snapshot logical volume {}: {}",
                    detail.snapshot_lv_path, err
                );
            }
            reset_mount_dir(root).await
        }
    }

    async fn ensure_clean_dir(path: &Path) -> BackupResult<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|err| BuckyBackupError::Failed(err.to_string()))?;
        }
        reset_mount_dir(path).await?;
        fs::create_dir_all(path)
            .await
            .map_err(|err| BuckyBackupError::Failed(err.to_string()))?;
        Ok(())
    }

    async fn find_mount_point(path: &Path) -> BackupResult<MountInfo> {
        let path_str = path.to_string_lossy();
        let output = run_command(
            "findmnt",
            &["-P", "-no", "SOURCE,TARGET", "--target", path_str.as_ref()],
        )
        .await?;
        let mut lines = output.lines();
        let line = lines.next().ok_or_else(|| {
            BuckyBackupError::Failed(format!(
                "findmnt returned empty result for {}",
                path.display()
            ))
        })?;
        let mut source = None;
        let mut target = None;
        for token in line.trim().split_whitespace() {
            if let Some(value) = token.strip_prefix("SOURCE=") {
                source = Some(unescape_findmnt_value(value));
            } else if let Some(value) = token.strip_prefix("TARGET=") {
                target = Some(unescape_findmnt_value(value));
            }
        }
        let source = source.ok_or_else(|| {
            BuckyBackupError::Failed(format!(
                "failed to parse mount source from findmnt output: {}",
                line
            ))
        })?;
        let target = target.ok_or_else(|| {
            BuckyBackupError::Failed(format!(
                "failed to parse mount target from findmnt output: {}",
                line
            ))
        })?;
        Ok(MountInfo {
            source,
            target: PathBuf::from(target),
        })
    }

    async fn describe_lv(device: &str) -> BackupResult<LvmInfo> {
        let output = run_command(
            "lvs",
            &[
                "--noheadings",
                "--separator=|",
                "-o",
                "vg_name,lv_name,lv_path",
                device,
            ],
        )
        .await
        .map_err(|err| {
            BuckyBackupError::Failed(format!(
                "failed to describe logical volume for {}: {}. Ensure the source path resides on an LVM logical volume before running snapshots.",
                device, err
            ))
        })?;
        let line = output
            .lines()
            .next()
            .ok_or_else(|| BuckyBackupError::Failed("lvs output is empty".to_string()))?;
        let parts: Vec<&str> = line.trim().split('|').collect();
        if parts.len() != 3 {
            return Err(BuckyBackupError::Failed(format!(
                "unexpected lvs output: {}",
                line
            )));
        }
        Ok(LvmInfo {
            vg_name: parts[0].trim().to_string(),
            lv_name: parts[1].trim().to_string(),
            lv_path: parts[2].trim().to_string(),
        })
    }

    async fn create_lvm_snapshot(origin_lv: &str, snapshot_name: &str) -> BackupResult<()> {
        let size_arg = format!("--extents={}", SNAPSHOT_EXTENTS);
        run_command(
            "lvcreate",
            &["--snapshot", "--name", snapshot_name, &size_arg, origin_lv],
        )
        .await
        .map(|_| ())
    }

    async fn mount_snapshot(snapshot_lv_path: &str, mountpoint: &Path) -> BackupResult<()> {
        run_command(
            "mount",
            &[
                "-o",
                "ro",
                snapshot_lv_path,
                mountpoint.to_string_lossy().as_ref(),
            ],
        )
        .await
        .map(|_| ())
    }

    async fn remove_snapshot_lv(lv_path: &str) -> Result<(), String> {
        run_command("lvremove", &["-f", lv_path])
            .await
            .map(|_| ())
            .map_err(|err| err.to_string())
    }

    pub(super) struct LinuxDmSnapshotBackend;

    #[derive(Debug, Serialize, Deserialize)]
    struct DmSnapshotMetadata {
        dm_name: String,
        loop_device: String,
        cow_file: String,
    }

    #[derive(Debug)]
    struct DmSnapshotResources {
        dm_name: String,
        snapshot_device: String,
        loop_device: String,
        cow_file: PathBuf,
    }

    const DM_SNAPSHOT_CHUNK_SIZE: u64 = 64;

    #[async_trait]
    impl SnapshotBackend for LinuxDmSnapshotBackend {
        fn id(&self) -> &'static str {
            "linux:dm-snapshot"
        }

        fn description(&self) -> &'static str {
            "Linux device-mapper snapshots"
        }

        fn platform(&self) -> SnapshotPlatform {
            SnapshotPlatform::Linux
        }

        async fn create_snapshot(
            &self,
            ctx: SnapshotCreateContext<'_>,
        ) -> BackupResult<BackendSnapshot> {
            let mount_root = ctx.candidate_root;
            let canonical = tokio::fs::canonicalize(ctx.source_path)
                .await
                .map_err(|err| {
                    warn!(
                        "create snapshot(backend: {}) failed for canonicalize: {:?}",
                        self.description(),
                        err
                    );
                    BuckyBackupError::Failed(err.to_string())
                })?;
            let mount_info = find_mount_point(&canonical).await.map_err(|err| {
                warn!(
                    "create snapshot(backend: {}) failed for find mount point: {:?}",
                    self.description(),
                    err
                );
                err
            })?;

            ensure_clean_dir(&mount_root).await.map_err(|err| {
                warn!(
                    "create snapshot(backend: {}) failed for clean dir: {:?}",
                    self.description(),
                    err
                );
                err
            })?;
            let dm_resources = match create_dm_snapshot(&mount_info.source, &mount_root).await {
                Ok(res) => res,
                Err(err) => {
                    warn!(
                        "create snapshot(backend: {}) failed for create dm snapshot: {:?}",
                        self.description(),
                        err
                    );
                    let _ = reset_mount_dir(&mount_root).await;
                    return Err(err);
                }
            };

            if let Err(err) = mount_snapshot(&dm_resources.snapshot_device, &mount_root).await {
                cleanup_dm_snapshot(&dm_resources).await;
                let _ = reset_mount_dir(&mount_root).await;

                warn!(
                    "create snapshot(backend: {}) failed for mount snapshot: {:?}",
                    self.description(),
                    err
                );
                return Err(err);
            }

            let relative = canonical
                .strip_prefix(&mount_info.target)
                .unwrap_or(Path::new(""));
            let final_path = if relative.as_os_str().is_empty() {
                mount_root.clone()
            } else {
                mount_root.join(relative)
            };

            let metadata = serde_json::to_value(DmSnapshotMetadata {
                dm_name: dm_resources.dm_name.clone(),
                loop_device: dm_resources.loop_device.clone(),
                cow_file: dm_resources.cow_file.to_string_lossy().to_string(),
            })
            .map_err(|err| {
                warn!(
                    "create snapshot(backend: {}) failed for serde json: {:?}",
                    self.description(),
                    err
                );
                BuckyBackupError::Failed(err.to_string())
            })?;

            info!(
                "created device-mapper snapshot {} mounted at {}",
                dm_resources.snapshot_device,
                mount_root.display()
            );

            Ok(BackendSnapshot {
                info: SnapshotInfo {
                    final_path,
                    root_path: mount_root,
                },
                metadata,
            })
        }

        async fn cleanup_snapshot(&self, root: &Path, metadata: Value) -> BackupResult<()> {
            let detail: DmSnapshotMetadata = serde_json::from_value(metadata)
                .map_err(|err| BuckyBackupError::Failed(err.to_string()))?;

            if root.exists() {
                if let Err(err) = run_command("umount", &[root.to_string_lossy().as_ref()]).await {
                    warn!(
                        "failed to unmount DM snapshot at {}: {}",
                        root.display(),
                        err
                    );
                }
            }

            cleanup_dm_artifacts(
                &detail.dm_name,
                &detail.loop_device,
                Path::new(&detail.cow_file),
            )
            .await;

            reset_mount_dir(root).await
        }
    }

    async fn create_dm_snapshot(
        origin: &str,
        workspace: &Path,
    ) -> BackupResult<DmSnapshotResources> {
        let sectors = query_block_sectors(origin).await.map_err(|err| {
            warn!("create dm snapshot failed for query sectors: {:?}", err);
            err
        })?;
        let cow_path = workspace.with_extension("cow");
        create_sparse_cow(&cow_path, sectors).await.map_err(|err| {
            warn!("create dm snapshot failed for create sparse cow: {:?}", err);
            err
        })?;

        let cow_path_string = cow_path.to_string_lossy().to_string();
        let loop_device_output =
            run_command("losetup", &["--find", "--show", cow_path_string.as_ref()])
                .await
                .map_err(|err| {
                    warn!("create dm snapshot failed for losetup: {:?}", err);
                    err
                })?;
        let loop_device = loop_device_output
            .lines()
            .next()
            .ok_or_else(|| BuckyBackupError::Failed("losetup returned empty output".to_string()))?
            .trim()
            .to_string();

        let table = format!(
            "0 {} snapshot {} {} P {}",
            sectors, origin, loop_device, DM_SNAPSHOT_CHUNK_SIZE
        );
        let dm_name = format!("bksdm_{}", short_id());
        let dm_name_arg = dm_name.as_str();
        let table_arg = table.as_str();
        run_command("dmsetup", &["create", dm_name_arg, "--table", table_arg])
            .await
            .map_err(|err| {
                warn!("create dm snapshot failed for dmsetup: {:?}", err);
                err
            })?;

        let snapshot_device = format!("/dev/mapper/{}", dm_name);

        Ok(DmSnapshotResources {
            dm_name,
            snapshot_device,
            loop_device,
            cow_file: cow_path,
        })
    }

    async fn cleanup_dm_snapshot(resources: &DmSnapshotResources) {
        cleanup_dm_artifacts(
            &resources.dm_name,
            &resources.loop_device,
            &resources.cow_file,
        )
        .await;
    }

    async fn cleanup_dm_artifacts(dm_name: &str, loop_device: &str, cow_path: &Path) {
        if let Err(err) = run_command("dmsetup", &["remove", dm_name]).await {
            warn!("failed to remove DM snapshot {}: {}", dm_name, err);
        }
        if let Err(err) = run_command("losetup", &["-d", loop_device]).await {
            warn!("failed to detach loop device {}: {}", loop_device, err);
        }
        match fs::remove_file(cow_path).await {
            Ok(_) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => {
                warn!(
                    "failed to remove DM snapshot COW file {}: {}",
                    cow_path.display(),
                    err
                );
            }
        }
    }

    async fn create_sparse_cow(path: &Path, sectors: u64) -> BackupResult<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|err| BuckyBackupError::Failed(err.to_string()))?;
        }
        let size_bytes = sectors
            .checked_mul(512)
            .ok_or_else(|| BuckyBackupError::Failed("cow file size overflow".to_string()))?;
        let size_arg = size_bytes.to_string();
        let path_str = path.to_string_lossy();
        run_command("truncate", &["-s", size_arg.as_str(), path_str.as_ref()])
            .await
            .map(|_| ())
    }

    async fn query_block_sectors(device: &str) -> BackupResult<u64> {
        let output = run_command("blockdev", &["--getsz", device]).await?;
        output.trim().parse::<u64>().map_err(|err| {
            BuckyBackupError::Failed(format!(
                "failed to parse blockdev output for {}: {}",
                device, err
            ))
        })
    }

    async fn run_command(cmd: &str, args: &[&str]) -> BackupResult<String> {
        let output =
            Command::new(cmd).args(args).output().await.map_err(|err| {
                BuckyBackupError::Failed(format!("failed to run {}: {}", cmd, err))
            })?;
        if !output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!(
                "snapshot run command failed, command: {} {:?} \n stdout: {} \n stderr: {}",
                cmd, args, stdout, stderr
            );
            return Err(BuckyBackupError::Failed(format!(
                "{} failed (code {:?}): {}{}{}",
                cmd,
                output.status.code(),
                stdout,
                if stderr.is_empty() { "" } else { " | " },
                stderr
            )));
        }
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}

#[cfg(target_os = "windows")]
mod windows {
    use super::*;
    use log::warn;
    use std::os::windows::fs::symlink_dir;
    use std::path::{Component, Path, Prefix};
    use tokio::process::Command;

    pub(super) struct WindowsVssBackend;

    #[derive(Debug, Serialize, Deserialize)]
    struct VssSnapshotMetadata {
        shadow_id: String,
    }

    #[derive(Debug)]
    struct ShadowInfo {
        shadow_id: String,
        shadow_volume_path: PathBuf,
    }

    #[async_trait]
    impl SnapshotBackend for WindowsVssBackend {
        fn id(&self) -> &'static str {
            "windows:vss"
        }

        fn description(&self) -> &'static str {
            "Windows VSS snapshots"
        }

        fn platform(&self) -> SnapshotPlatform {
            SnapshotPlatform::Windows
        }

        async fn create_snapshot(
            &self,
            ctx: SnapshotCreateContext<'_>,
        ) -> BackupResult<BackendSnapshot> {
            let mount_root = ctx.candidate_root;
            let absolute = absolute_path(ctx.source_path)?;
            let normalized = normalize_drive_path(&absolute);
            let (volume_label, volume_root) = volume_from_path(&normalized)?;
            ensure_clean_dir(&mount_root).await?;

            let shadow = create_shadow_copy(&volume_label).await?;
            let target = ensure_trailing_separator(&shadow.shadow_volume_path);
            if let Err(err) = symlink_dir(&target, &mount_root) {
                let _ = delete_shadow_copy(&shadow.shadow_id).await;
                return Err(BuckyBackupError::Failed(format!(
                    "failed to link VSS snapshot to {}: {}",
                    mount_root.display(),
                    err
                )));
            }
            let relative = normalized
                .strip_prefix(&volume_root)
                .map_err(|err| BuckyBackupError::Failed(err.to_string()))?;
            let final_path = if relative.as_os_str().is_empty() {
                mount_root.clone()
            } else {
                mount_root.join(relative)
            };

            let metadata = serde_json::to_value(VssSnapshotMetadata {
                shadow_id: shadow.shadow_id.clone(),
            })
            .map_err(|err| BuckyBackupError::Failed(err.to_string()))?;

            info!(
                "created VSS snapshot {} mounted at {}",
                shadow.shadow_id,
                mount_root.display()
            );

            Ok(BackendSnapshot {
                info: SnapshotInfo {
                    final_path,
                    root_path: mount_root,
                },
                metadata,
            })
        }

        async fn cleanup_snapshot(&self, root: &Path, metadata: Value) -> BackupResult<()> {
            let detail: VssSnapshotMetadata = serde_json::from_value(metadata)
                .map_err(|err| BuckyBackupError::Failed(err.to_string()))?;
            if let Err(err) = delete_shadow_copy(&detail.shadow_id).await {
                warn!(
                    "failed to delete VSS snapshot {}: {}",
                    detail.shadow_id, err
                );
            }

            reset_mount_dir(root).await
        }
    }

    fn absolute_path(path: &Path) -> BackupResult<PathBuf> {
        if path.is_absolute() {
            return Ok(path.to_path_buf());
        }
        let cwd =
            std::env::current_dir().map_err(|err| BuckyBackupError::Failed(err.to_string()))?;
        Ok(cwd.join(path))
    }

    fn volume_from_path(path: &Path) -> BackupResult<(String, PathBuf)> {
        let mut components = path.components();
        match components.next() {
            Some(Component::Prefix(prefix)) => match prefix.kind() {
                Prefix::Disk(letter) | Prefix::VerbatimDisk(letter) => {
                    let drive_letter = letter as char;
                    let label = format!("{}:", drive_letter);
                    let root = PathBuf::from(format!("{}\\", label));
                    Ok((label, root))
                }
                _ => Err(BuckyBackupError::Failed(format!(
                    "unsupported prefix in path: {}",
                    path.display()
                ))),
            },
            _ => Err(BuckyBackupError::Failed(format!(
                "cannot determine drive for {}",
                path.display()
            ))),
        }
    }

    async fn create_shadow_copy(volume_label: &str) -> BackupResult<ShadowInfo> {
        let output = run_command("vssadmin", &["create", "shadow", "/for", volume_label]).await?;
        parse_shadow_output(&output)
    }

    async fn delete_shadow_copy(shadow_id: &str) -> Result<(), String> {
        run_command(
            "vssadmin",
            &["delete", "shadows", "/shadow", shadow_id, "/quiet"],
        )
        .await
        .map(|_| ())
        .map_err(|err| err.to_string())
    }

    fn parse_shadow_output(output: &str) -> BackupResult<ShadowInfo> {
        let mut shadow_id = None;
        let mut shadow_volume = None;
        for line in output.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("Shadow Copy ID:") {
                shadow_id = trimmed.split(':').nth(1).map(|s| s.trim().to_string());
            } else if trimmed.starts_with("Shadow Copy Volume:") {
                let value = trimmed.split(':').nth(1).map(|s| s.trim().to_string());
                if let Some(val) = value {
                    shadow_volume = Some(PathBuf::from(val));
                }
            }
        }

        let id = shadow_id
            .ok_or_else(|| BuckyBackupError::Failed("failed to parse VSS shadow id".to_string()))?;
        let volume_path = shadow_volume.ok_or_else(|| {
            BuckyBackupError::Failed("failed to parse VSS shadow volume path".to_string())
        })?;

        Ok(ShadowInfo {
            shadow_id: id,
            shadow_volume_path: volume_path,
        })
    }

    async fn ensure_clean_dir(path: &Path) -> BackupResult<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|err| BuckyBackupError::Failed(err.to_string()))?;
        }
        reset_mount_dir(path).await
    }

    async fn run_command(cmd: &str, args: &[&str]) -> BackupResult<String> {
        let output =
            Command::new(cmd).args(args).output().await.map_err(|err| {
                BuckyBackupError::Failed(format!("failed to run {}: {}", cmd, err))
            })?;
        if !output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(BuckyBackupError::Failed(format!(
                "{} failed (code {:?}): {}{}{}",
                cmd,
                output.status.code(),
                stdout,
                if stderr.is_empty() { "" } else { " | " },
                stderr
            )));
        }
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    fn normalize_drive_path(path: &Path) -> PathBuf {
        let raw = path.as_os_str().to_string_lossy();
        if let Some(stripped) = raw.strip_prefix(r"\\?\") {
            PathBuf::from(stripped)
        } else {
            path.to_path_buf()
        }
    }

    fn ensure_trailing_separator(path: &Path) -> PathBuf {
        let mut text = path.as_os_str().to_string_lossy().to_string();
        if !text.ends_with('\\') {
            text.push('\\');
        }
        PathBuf::from(text)
    }
}

#[cfg(target_os = "linux")]
static LINUX_LVM_BACKEND: linux::LinuxLvmBackend = linux::LinuxLvmBackend;

#[cfg(target_os = "linux")]
static LINUX_DM_BACKEND: linux::LinuxDmSnapshotBackend = linux::LinuxDmSnapshotBackend;

#[cfg(target_os = "windows")]
static WINDOWS_VSS_BACKEND: windows::WindowsVssBackend = windows::WindowsVssBackend;
