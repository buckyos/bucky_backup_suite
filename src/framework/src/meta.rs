use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::{
    checkpoint::{DirChildType, FileStreamReader, StorageReader},
    engine::TaskUuid,
    error::{BackupError, BackupResult},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PreserveStateId(u64);

impl Into<u64> for PreserveStateId {
    fn into(self) -> u64 {
        self.0
    }
}

impl From<u64> for PreserveStateId {
    fn from(id: u64) -> Self {
        Self(id)
    }
}

// All times should use UTC time. Unless otherwise stated, they should be accurate to the second.

pub trait MetaBound: Clone + Serialize + for<'de> Deserialize<'de> {}

impl<T> MetaBound for T where T: Clone + Serialize + for<'de> Deserialize<'de> {}

#[derive(Clone, Serialize, Deserialize)]
pub struct CheckPointMeta<
    ServiceCheckPointMeta: MetaBound,
    ServiceDirMetaType: MetaBound,
    ServiceFileMetaType: MetaBound,
    ServiceLinkMetaType: MetaBound,
    ServiceLogMetaType: MetaBound,
> {
    pub task_friendly_name: String,
    pub task_uuid: TaskUuid,
    pub version: CheckPointVersion,
    pub prev_versions: Vec<CheckPointVersion>, // all versions this checkpoint depends on
    pub create_time: SystemTime,
    pub complete_time: SystemTime,
    #[serde(
        serialize_with = "serialize_meta_bound",
        deserialize_with = "deserialize_meta_bound"
    )]
    pub root: StorageItem<
        ServiceDirMetaType,
        ServiceFileMetaType,
        ServiceLinkMetaType,
        ServiceLogMetaType,
    >,

    // space size
    pub occupied_size: u64, // The really size the data described by this object occupied in the storage.
    pub consume_size: u64, // The size the data described by this object consumed in the storage, `consume_size` is greater than `occupied_size`, when the storage unit size is greater than 1 byte.
    pub all_prev_version_occupied_size: u64,
    pub all_prev_version_consume_size: u64,

    // Special for services
    #[serde(
        serialize_with = "serialize_meta_bound",
        deserialize_with = "deserialize_meta_bound"
    )]
    pub service_meta: Option<ServiceCheckPointMeta>,
}

impl<
        ServiceCheckPointMeta: MetaBound,
        ServiceDirMetaType: MetaBound,
        ServiceFileMetaType: MetaBound,
        ServiceLinkMetaType: MetaBound,
        ServiceLogMetaType: MetaBound,
    >
    CheckPointMeta<
        ServiceCheckPointMeta,
        ServiceDirMetaType,
        ServiceFileMetaType,
        ServiceLinkMetaType,
        ServiceLogMetaType,
    >
{
    pub fn combine_previous_versions(prev_checkpoints: &[&Self]) -> BackupResult<Self> {
        if prev_checkpoints.is_empty() {
            return Err(BackupError::InvalidArgument(
                "prev_checkpoints is empty".to_string(),
            ));
        }

        // sort by previous versions
        let mut prev_checkpoints = Vec::from(prev_checkpoints);
        Self::sort_checkpoints(prev_checkpoints.as_mut_slice());

        let first_checkpoint = prev_checkpoints.first().unwrap();

        let mut root_dir = DirectoryMeta {
            name: PathBuf::from("/"),
            attributes: first_checkpoint.root.attributes().clone(),
            service_meta: None,
            children: vec![],
        };

        let mut delta_root_dir = DirectoryMeta {
            name: PathBuf::from("/"),
            attributes: first_checkpoint.root.attributes().clone(),
            service_meta: None,
            children: vec![],
        };

        for checkpoint in prev_checkpoints.iter() {
            // add each `root` item to the meta
            let delta_dir = loop {
                if let StorageItem::Dir(dir, _) = &checkpoint.root {
                    if dir.name() == PathBuf::from("/") {
                        break dir;
                    }
                }
                delta_root_dir.attributes = root_dir.attributes().clone();
                delta_root_dir.children = vec![checkpoint.root.clone()];
                break &delta_root_dir;
            };

            root_dir.add_delta(delta_dir)
        }

        let last_checkpoint = prev_checkpoints.last().unwrap();

        // clone from the last checkpoint
        Ok(Self {
            task_friendly_name: last_checkpoint.task_friendly_name.clone(),
            task_uuid: last_checkpoint.task_uuid,
            version: last_checkpoint.version,
            prev_versions: prev_checkpoints.iter().map(|c| c.version).collect(),
            create_time: last_checkpoint.create_time.clone(),
            complete_time: last_checkpoint.complete_time.clone(),
            root: StorageItem::Dir(root_dir, vec![]),
            occupied_size: last_checkpoint.occupied_size,
            consume_size: last_checkpoint.consume_size,
            all_prev_version_occupied_size: last_checkpoint.all_prev_version_occupied_size,
            all_prev_version_consume_size: last_checkpoint.all_prev_version_consume_size,
            service_meta: last_checkpoint.service_meta.clone(),
        })
    }

    pub fn sort_checkpoints(checkpoints: &mut [&Self]) {
        for i in 0..checkpoints.len() {
            let mut is_swapped = true;
            while is_swapped {
                is_swapped = false;
                for j in i + 1..checkpoints.len() {
                    if checkpoints[i]
                        .prev_versions
                        .iter()
                        .find(|v| **v == checkpoints[j].version)
                        .is_some()
                    {
                        checkpoints.swap(i, j);
                        is_swapped = true;
                    }
                }
            }
        }
    }

    pub fn estimate_occupy_size(&self) -> u64 {
        // meta size
        let mut occupy_size = serde_json::to_string(self).unwrap().len() as u64;
        // calculate the size of all files(Type: DirChildType::File) in the meta.
        // Traverse it as meta_from_reader

        let mut wait_read_dirs = vec![&self.root];

        while !wait_read_dirs.is_empty() {
            let current_item = wait_read_dirs.remove(0);

            match current_item {
                StorageItem::Dir(dir, _) => {
                    for child in dir.children.iter() {
                        match child {
                            StorageItem::Dir(_, _) => {
                                wait_read_dirs.push(child);
                            }
                            StorageItem::File(file, _) => {
                                occupy_size += file.size;
                            }
                            StorageItem::Link(_, _) => {}
                            StorageItem::Log(_, _) => {}
                            StorageItem::FileDiff(diff, _) => {
                                occupy_size +=
                                    diff.diff_chunks.iter().map(|diff| diff.length).sum::<u64>();
                            }
                        }
                    }
                }
                StorageItem::File(file, _) => {
                    occupy_size += file.size;
                }
                StorageItem::Link(_, _) => {}
                StorageItem::Log(_, _) => {}
                StorageItem::FileDiff(diff, _) => {
                    occupy_size += diff.diff_chunks.iter().map(|diff| diff.length).sum::<u64>()
                }
            }
        }

        occupy_size
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CheckPointVersion {
    #[serde(
        serialize_with = "serialize_system_time",
        deserialize_with = "deserialize_system_time"
    )]
    pub time: SystemTime,
    pub seq: u64,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StorageItemAttributes {
    #[serde(
        serialize_with = "serialize_system_time",
        deserialize_with = "deserialize_system_time"
    )]
    pub create_time: SystemTime,
    #[serde(
        serialize_with = "serialize_system_time",
        deserialize_with = "deserialize_system_time"
    )]
    pub last_update_time: SystemTime,
    pub owner: String,
    pub group: String,
    pub permissions: String,
}

// (meta, logs), logs record the change history of stored files (not other types) for this item (if any),
// mainly used for incremental synchronization and reuse of files, and are rarely used in most cases,
// logs are not serialized or stored, they are only used in memory
#[derive(Clone)]
pub enum StorageItem<
    ServiceDirMetaType: MetaBound,
    ServiceFileMetaType: MetaBound,
    ServiceLinkMetaType: MetaBound,
    ServiceLogMetaType: MetaBound,
> {
    Dir(
        DirectoryMeta<
            ServiceDirMetaType,
            ServiceFileMetaType,
            ServiceLinkMetaType,
            ServiceLogMetaType,
        >,
        Vec<Option<FileLog<ServiceFileMetaType>>>,
    ),
    File(
        FileMeta<ServiceFileMetaType>,
        Vec<Option<FileLog<ServiceFileMetaType>>>,
    ),
    FileDiff(
        FileDiffMeta<ServiceFileMetaType>,
        Vec<Option<FileLog<ServiceFileMetaType>>>,
    ),
    Link(
        LinkMeta<ServiceLinkMetaType>,
        Vec<Option<FileLog<ServiceFileMetaType>>>,
    ),
    Log(
        LogMeta<ServiceLogMetaType>,
        Vec<Option<FileLog<ServiceFileMetaType>>>,
    ),
}

impl<
        ServiceDirMetaType: MetaBound,
        ServiceFileMetaType: MetaBound,
        ServiceLinkMetaType: MetaBound,
        ServiceLogMetaType: MetaBound,
    > Serialize
    for StorageItem<
        ServiceDirMetaType,
        ServiceFileMetaType,
        ServiceLinkMetaType,
        ServiceLogMetaType,
    >
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Serialize `self` into the format `Type:Serialize::serialize(content)`
        match self {
            StorageItem::Dir(dir, _) => serializer
                .serialize_str(format!("DIR:{}", serde_json::to_string(dir).unwrap()).as_str()),
            StorageItem::File(file, _) => serializer
                .serialize_str(format!("FILE:{}", serde_json::to_string(file).unwrap()).as_str()),
            StorageItem::Link(link, _) => serializer
                .serialize_str(format!("LINK:{}", serde_json::to_string(link).unwrap()).as_str()),
            StorageItem::Log(log, _) => serializer
                .serialize_str(format!("LOG:{}", serde_json::to_string(log).unwrap()).as_str()),
            StorageItem::FileDiff(diff, _) => serializer
                .serialize_str(format!("DIFF:{}", serde_json::to_string(diff).unwrap()).as_str()),
        }
    }
}

impl<
        'de,
        ServiceDirMetaType: MetaBound,
        ServiceFileMetaType: MetaBound,
        ServiceLinkMetaType: MetaBound,
        ServiceLogMetaType: MetaBound,
    > Deserialize<'de>
    for StorageItem<
        ServiceDirMetaType,
        ServiceFileMetaType,
        ServiceLinkMetaType,
        ServiceLogMetaType,
    >
{
    fn deserialize<D>(
        deserializer: D,
    ) -> Result<
        StorageItem<
            ServiceDirMetaType,
            ServiceFileMetaType,
            ServiceLinkMetaType,
            ServiceLogMetaType,
        >,
        D::Error,
    >
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let v: Vec<&str> = s.splitn(2, ':').collect();
        if v.len() != 2 {
            return Err(serde::de::Error::custom(format!(
                "Unknown storage item: {}",
                s
            )));
        }
        match v[0] {
            "DIR" => {
                let dir: DirectoryMeta<
                    ServiceDirMetaType,
                    ServiceFileMetaType,
                    ServiceLinkMetaType,
                    ServiceLogMetaType,
                > = serde_json::from_str(v[1]).unwrap();
                Ok(StorageItem::Dir(dir, vec![]))
            }
            "FILE" => {
                let file: FileMeta<ServiceFileMetaType> = serde_json::from_str(v[1]).unwrap();
                Ok(StorageItem::File(file, vec![]))
            }
            "LINK" => {
                let link: LinkMeta<ServiceLinkMetaType> = serde_json::from_str(v[1]).unwrap();
                Ok(StorageItem::Link(link, vec![]))
            }
            "LOG" => {
                let log: LogMeta<ServiceLogMetaType> = serde_json::from_str(v[1]).unwrap();
                Ok(StorageItem::Log(log, vec![]))
            }
            "DIFF" => {
                let diff: FileDiffMeta<ServiceFileMetaType> = serde_json::from_str(v[1]).unwrap();
                Ok(StorageItem::FileDiff(diff, vec![]))
            }
            _ => Err(serde::de::Error::custom(format!(
                "Unknown storage item: {}",
                s
            ))),
        }
    }
}

impl<
        ServiceDirMetaType: MetaBound,
        ServiceFileMetaType: MetaBound,
        ServiceLinkMetaType: MetaBound,
        ServiceLogMetaType: MetaBound,
    > StorageItemField
    for StorageItem<
        ServiceDirMetaType,
        ServiceFileMetaType,
        ServiceLinkMetaType,
        ServiceLogMetaType,
    >
{
    fn name(&self) -> &Path {
        match self {
            StorageItem::Dir(d, _) => d.name(),
            StorageItem::File(f, _) => f.name(),
            StorageItem::Link(l, _) => l.name(),
            StorageItem::Log(l, _) => l.name(),
            StorageItem::FileDiff(d, _) => d.name(),
        }
    }

    fn attributes(&self) -> &StorageItemAttributes {
        match self {
            StorageItem::Dir(d, _) => d.attributes(),
            StorageItem::File(f, _) => f.attributes(),
            StorageItem::Link(l, _) => l.attributes(),
            StorageItem::Log(l, _) => l.attributes(),
            StorageItem::FileDiff(d, _) => d.attributes(),
        }
    }

    fn attributes_mut(&mut self) -> &mut StorageItemAttributes {
        match self {
            StorageItem::Dir(d, _) => d.attributes_mut(),
            StorageItem::File(f, _) => f.attributes_mut(),
            StorageItem::Link(l, _) => l.attributes_mut(),
            StorageItem::Log(l, _) => l.attributes_mut(),
            StorageItem::FileDiff(d, _) => d.attributes_mut(),
        }
    }
}

impl<
        ServiceDirMetaType: MetaBound,
        ServiceFileMetaType: MetaBound,
        ServiceLinkMetaType: MetaBound,
        ServiceLogMetaType: MetaBound,
    >
    StorageItem<ServiceDirMetaType, ServiceFileMetaType, ServiceLinkMetaType, ServiceLogMetaType>
{
    fn logs(&self) -> &[Option<FileLog<ServiceFileMetaType>>] {
        match self {
            StorageItem::Dir(_, logs) => logs.as_slice(),
            StorageItem::File(_, logs) => logs.as_slice(),
            StorageItem::Link(_, logs) => logs.as_slice(),
            StorageItem::Log(_, logs) => logs.as_slice(),
            StorageItem::FileDiff(_, logs) => logs.as_slice(),
        }
    }

    fn logs_mut(&mut self) -> &mut Vec<Option<FileLog<ServiceFileMetaType>>> {
        match self {
            StorageItem::Dir(_, logs) => logs,
            StorageItem::File(_, logs) => logs,
            StorageItem::Link(_, logs) => logs,
            StorageItem::Log(_, logs) => logs,
            StorageItem::FileDiff(_, logs) => logs,
        }
    }

    fn find_log(&self, hash: &str, size: u64) -> Option<&FileLog<ServiceFileMetaType>> {
        self.logs()
            .iter()
            .find(|log| {
                log.as_ref()
                    .map_or(false, |log| log.hash == hash && log.size == size)
            })
            .map_or(None, |log| log.as_ref())
    }

    // return true if there is a file in history but it has been removed or replaced with other type.
    // if the item is removed, there will be a log-item with action `Remove`.
    fn is_file_removed(&self) -> bool {
        self.logs().last().map_or(false, |log| log.is_none())
    }

    fn is_file_in_history(&self) -> bool {
        self.logs().len() > 0
    }

    fn take_logs(&mut self) -> Vec<Option<FileLog<ServiceFileMetaType>>> {
        std::mem::take(self.logs_mut())
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct DirectoryMeta<
    ServiceDirMetaType: MetaBound,
    ServiceFileMetaType: MetaBound,
    ServiceLinkMetaType: MetaBound,
    ServiceLogMetaType: MetaBound,
> {
    pub name: PathBuf,
    pub attributes: StorageItemAttributes,
    #[serde(
        serialize_with = "serialize_meta_bound",
        deserialize_with = "deserialize_meta_bound"
    )]
    pub service_meta: Option<ServiceDirMetaType>,
    #[serde(
        serialize_with = "serialize_meta_bound",
        deserialize_with = "deserialize_meta_bound"
    )]
    pub children: Vec<
        StorageItem<
            ServiceDirMetaType,
            ServiceFileMetaType,
            ServiceLinkMetaType,
            ServiceLogMetaType,
        >,
    >,
}

impl<
        ServiceDirMetaType: MetaBound,
        ServiceFileMetaType: MetaBound,
        ServiceLinkMetaType: MetaBound,
        ServiceLogMetaType: MetaBound,
    > StorageItemField
    for DirectoryMeta<
        ServiceDirMetaType,
        ServiceFileMetaType,
        ServiceLinkMetaType,
        ServiceLogMetaType,
    >
{
    fn name(&self) -> &Path {
        &self.name
    }

    fn attributes(&self) -> &StorageItemAttributes {
        &self.attributes
    }

    fn attributes_mut(&mut self) -> &mut StorageItemAttributes {
        &mut self.attributes
    }
}

impl<
        ServiceDirMetaType: MetaBound,
        ServiceFileMetaType: MetaBound,
        ServiceLinkMetaType: MetaBound,
        ServiceLogMetaType: MetaBound,
    >
    DirectoryMeta<ServiceDirMetaType, ServiceFileMetaType, ServiceLinkMetaType, ServiceLogMetaType>
{
    pub async fn from_reader(reader: &dyn StorageReader) -> BackupResult<Self> {
        // Traverse all hierarchical subdirectories of reader. read-dir() in a breadth first manner and store them in the structure of CheckPointMeta.
        // For each directory, call read_dir() to get the list of files and subdirectories in the directory.
        // For each file, call read_file() to get the file content.
        // For each subdirectory, call read_dir() recursively.
        // For each link, call read_link() to get the link information.
        // For each file, call stat() to get the file attributes.
        // Return the CheckPointMeta structure.

        let root_name = PathBuf::from("/");
        let root_dir_attr = reader.stat(&root_name).await?;
        let root_dir = Self {
            name: root_name.clone(),
            attributes: root_dir_attr,
            service_meta: None,
            children: vec![],
        };

        let mut all_read_files = vec![(None, StorageItem::Dir(root_dir, vec![]), root_name)]; // <parent-index, item, full-path>
        let mut next_dir_index = 0;

        while next_dir_index < all_read_files.len() {
            let parent_index = next_dir_index;
            let parent_full_path = all_read_files[next_dir_index].2.as_path();
            let current_item = &all_read_files[next_dir_index].1;
            next_dir_index += 1;
            if let StorageItem::Dir(_, _) = current_item {
            } else {
                continue;
            }

            let mut dir_reader = reader.read_dir(parent_full_path).await?;

            while let Some(child) = dir_reader.next().await? {
                let parent_full_path = all_read_files[next_dir_index].2.as_path();
                let child_name = child.path().to_path_buf();
                let full_path = parent_full_path.join(&child_name);
                let child_attr = reader.stat(&child_name).await?;
                let child_meta = match child {
                    DirChildType::File(_) => {
                        let file_size = reader.file_size(&child_name).await?;
                        let hash =
                            FileStreamReader::new(reader, full_path.as_path(), 0, 1024 * 1024)
                                .hash()
                                .await?;
                        StorageItem::File(
                            FileMeta {
                                name: child_name,
                                attributes: child_attr,
                                service_meta: None,
                                hash,
                                size: file_size,
                                upload_bytes: 0,
                            },
                            vec![],
                        )
                    }
                    DirChildType::Dir(_) => StorageItem::Dir(
                        DirectoryMeta {
                            name: child_name,
                            attributes: child_attr,
                            service_meta: None,
                            children: vec![],
                        },
                        vec![],
                    ),
                    DirChildType::Link(_) => {
                        let link_info = reader.read_link(&child_name).await?;
                        StorageItem::Link(
                            LinkMeta {
                                name: child_name,
                                attributes: child_attr,
                                service_meta: None,
                                is_hard: link_info.is_hard,
                                target: link_info.target,
                            },
                            vec![],
                        )
                    }
                };

                all_read_files.push((Some(parent_index), child_meta, full_path));
            }
        }

        loop {
            let (parent_index, child_meta, _) = all_read_files
                .pop()
                .expect("root directory should be in the list");
            if let Some(parent_index) = parent_index {
                let parent = &mut all_read_files[parent_index].1;
                if let StorageItem::Dir(parent_dir, _) = parent {
                    parent_dir.children.push(child_meta);
                } else {
                    unreachable!("only directory can have children");
                }
            } else if let StorageItem::Dir(child_meta, _) = child_meta {
                return Ok(child_meta);
            }
        }
    }
}

#[derive(Clone)]
pub struct FileLog<ServiceFileMetaType: MetaBound> {
    pub service_meta: Option<ServiceFileMetaType>,
    pub hash: String,
    pub size: u64,
}

impl<
        ServiceDirMetaType: MetaBound,
        ServiceFileMetaType: MetaBound,
        ServiceLinkMetaType: MetaBound,
        ServiceLogMetaType: MetaBound,
    >
    DirectoryMeta<ServiceDirMetaType, ServiceFileMetaType, ServiceLinkMetaType, ServiceLogMetaType>
{
    // meta = meta(reader) - meta(base_meta)
    pub async fn delta_from_reader(
        base_meta: &StorageItem<
            ServiceDirMetaType,
            ServiceFileMetaType,
            ServiceLinkMetaType,
            ServiceLogMetaType,
        >,
        base_reader: &dyn StorageReader,
        reader: &dyn StorageReader,
    ) -> BackupResult<
        DirectoryMeta<
            ServiceDirMetaType,
            ServiceFileMetaType,
            ServiceLinkMetaType,
            ServiceLogMetaType,
        >,
    > {
        let root_name = PathBuf::from("/");
        let root_dir_attr = reader.stat(&root_name).await?;
        let root_dir = DirectoryMeta {
            name: root_name.clone(),
            attributes: root_dir_attr,
            service_meta: None,
            children: vec![],
        };

        // makesource there is a directory in base.
        let mut base_root_dir = DirectoryMeta {
            name: root_name.clone(),
            attributes: StorageItemAttributes {
                create_time: SystemTime::UNIX_EPOCH,
                last_update_time: SystemTime::UNIX_EPOCH,
                owner: "".to_string(),
                group: "".to_string(),
                permissions: "".to_string(),
            },
            service_meta: None,
            children: vec![],
        };
        let base_meta = if let StorageItem::Dir(base_dir, _) = base_meta {
            base_dir
        } else {
            base_root_dir.children.push(base_meta.clone());
            &base_root_dir
        };

        let mut all_read_files = vec![(None, StorageItem::Dir(root_dir, vec![]), root_name)]; // <parent-index, item, full-path>
        let mut all_reader_dir_indexs = vec![(0, true)]; // <index-in-all_read_files, is-updated>
        let mut all_base_dirs = vec![Some(base_meta)];
        let mut next_dir_index = 0;

        while next_dir_index < all_reader_dir_indexs.len() {
            let current_base_dir = all_base_dirs[next_dir_index];
            let (parent_index, _) = all_reader_dir_indexs[next_dir_index];
            let parent_full_path = all_read_files[parent_index].2.clone();
            next_dir_index += 1;

            let mut dir_reader = reader.read_dir(parent_full_path.as_path()).await?;
            let mut children = HashSet::new();

            while let Some(child) = dir_reader.next().await? {
                let child_name = child.path().to_path_buf();
                let full_path = parent_full_path.join(&child_name);

                children.insert(child_name.clone());

                let base_child = current_base_dir.map_or(None, |dir| {
                    dir.children
                        .iter()
                        .find(|c| c.name() == child_name.as_path())
                });

                let child_attr = reader.stat(&child_name).await?;

                // compare attributes
                let mut update_attr_log = None;
                if let Some(base_child) = base_child {
                    if base_child.attributes() != &child_attr {
                        update_attr_log = Some(LogMeta {
                            name: child_name.clone(),
                            attributes: child_attr.clone(),
                            service_meta: None,
                            action: LogAction::UpdateAttributes,
                        })
                    }
                }

                let child_meta = match child {
                    DirChildType::File(_) => {
                        let file_size = reader.file_size(&child_name).await?;
                        let hash =
                            FileStreamReader::new(reader, full_path.as_path(), 0, 1024 * 1024)
                                .hash()
                                .await?;
                        let (is_no_modify, use_diff) = match base_child {
                            Some(base_child) => {
                                match base_child {
                                    StorageItem::Dir(_, _) => {
                                        // use exist file if it's same, otherwise create a new file
                                        (false, false)
                                    }
                                    StorageItem::File(base_file, _) => {
                                        // ignore it if it's same, otherwise use exist, and diff at last
                                        (
                                            base_file.hash == hash && base_file.size == file_size,
                                            true,
                                        )
                                    }
                                    StorageItem::FileDiff(diff, _) => {
                                        // ignore it if it's same, otherwise use exist, and diff at last
                                        (diff.hash == hash && diff.size == file_size, true)
                                    }
                                    StorageItem::Link(_, _) => {
                                        // use exist file if it's same, otherwise create a new file
                                        (false, false)
                                    }
                                    StorageItem::Log(_, _) => {
                                        // use exist file if it's same, otherwise create a new file
                                        (false, false)
                                    }
                                }
                            }
                            None => (false, false),
                        };

                        if is_no_modify {
                            None
                        } else {
                            // find from all files in the directory, if it's exists in earlier time, use the meta from service simple.
                            let exist_file_meta = base_root_dir.find_file_service_meta(
                                hash.as_str(),
                                file_size,
                                true,
                            );
                            if let Some(exist_file_meta) = exist_file_meta {
                                Some(StorageItem::File(
                                    FileMeta {
                                        name: child_name,
                                        attributes: child_attr,
                                        service_meta: exist_file_meta.clone(), // the `target` can use the history meta to reuse the space, and it should save it in a new space.
                                        hash,
                                        size: file_size,
                                        upload_bytes: 0,
                                    },
                                    vec![],
                                ))
                            } else if use_diff {
                                // diff
                                let diffs = FileDiffChunk::from_reader(
                                    &mut FileStreamReader::new(
                                        base_reader,
                                        full_path.as_path(),
                                        0,
                                        1024 * 1024,
                                    ),
                                    &mut FileStreamReader::new(
                                        reader,
                                        full_path.as_path(),
                                        0,
                                        1024 * 1024,
                                    ),
                                )
                                .await?;
                                Some(StorageItem::FileDiff(
                                    FileDiffMeta {
                                        name: child_name,
                                        attributes: child_attr,
                                        service_meta: None,
                                        hash,
                                        size: file_size,
                                        upload_bytes: 0,
                                        diff_chunks: diffs,
                                    },
                                    vec![],
                                ))
                            } else {
                                // new file
                                Some(StorageItem::File(
                                    FileMeta {
                                        name: child_name,
                                        attributes: child_attr,
                                        service_meta: None,
                                        hash,
                                        size: file_size,
                                        upload_bytes: 0,
                                    },
                                    vec![],
                                ))
                            }
                        }
                    }
                    DirChildType::Dir(_) => {
                        let is_type_changed = match base_child {
                            Some(base_child) => {
                                if let StorageItem::Dir(base_child, _) = base_child {
                                    all_base_dirs.push(Some(base_child));
                                    false
                                } else {
                                    all_base_dirs.push(None);
                                    true
                                }
                            }
                            None => {
                                all_base_dirs.push(None);
                                true
                            }
                        };
                        all_reader_dir_indexs.push((
                            all_read_files.len(),
                            is_type_changed || update_attr_log.is_some(),
                        ));
                        Some(StorageItem::Dir(
                            DirectoryMeta {
                                name: child_name,
                                attributes: child_attr,
                                service_meta: None,
                                children: vec![],
                            },
                            vec![],
                        ))
                    }
                    DirChildType::Link(_) => {
                        let link_info = reader.read_link(&child_name).await?;
                        let mut is_updated = true;
                        if let Some(base_child) = base_child {
                            if let StorageItem::Link(base_link, _) = base_child {
                                if base_link.is_hard == link_info.is_hard
                                    && base_link.target == link_info.target
                                {
                                    // no update
                                    is_updated = false;
                                }
                            }
                        }

                        if is_updated {
                            Some(StorageItem::Link(
                                LinkMeta {
                                    name: child_name,
                                    attributes: child_attr,
                                    service_meta: None,
                                    is_hard: link_info.is_hard,
                                    target: link_info.target,
                                },
                                vec![],
                            ))
                        } else {
                            None
                        }
                    }
                };

                // next_dir_index has +1 in the loop
                if let Some(child_meta) = child_meta {
                    all_read_files.push((Some(next_dir_index - 1), child_meta, full_path));
                } else if let Some(update_attr_log) = update_attr_log {
                    all_read_files.push((
                        Some(next_dir_index - 1),
                        StorageItem::Log(update_attr_log, vec![]),
                        full_path,
                    ));
                }
            }

            if let Some(base_dir) = current_base_dir {
                for child in base_dir.children.iter() {
                    // add LogAction::Remove for the `child`` not in `children`.
                    if !children.contains(child.name()) {
                        all_read_files.push((
                            Some(next_dir_index - 1),
                            StorageItem::Log(
                                LogMeta {
                                    name: child.name().to_path_buf(),
                                    attributes: child.attributes().clone(),
                                    service_meta: None,
                                    action: LogAction::Remove,
                                },
                                vec![],
                            ),
                            parent_full_path.join(child.name()),
                        ));
                    }
                }
            }
        }

        loop {
            let (parent_index, child_meta, _) = all_read_files
                .pop()
                .expect("root directory should be in the list");
            if let Some(parent_index) = parent_index {
                let parent_index = if parent_index == all_reader_dir_indexs.len() - 1 {
                    let (parent_index, is_updated) =
                        all_reader_dir_indexs.pop().expect("dir should existed");
                    if !is_updated {
                        continue;
                    }
                    parent_index
                } else {
                    all_reader_dir_indexs[parent_index].1 = true;
                    let (parent_index, _) = all_reader_dir_indexs[parent_index];
                    parent_index
                };

                let parent = &mut all_read_files[parent_index].1;
                if let StorageItem::Dir(parent_dir, _) = parent {
                    parent_dir.children.push(child_meta);
                } else {
                    unreachable!("only directory can have children");
                }
            } else if let StorageItem::Dir(child_meta, _) = child_meta {
                return Ok(child_meta);
            }
        }
    }

    // self += delta
    // the name of delta should be same as self.
    pub fn add_delta(&mut self, delta: &Self) {
        if delta.name() != self.name() {
            panic!("delta directory name should be same as self");
        }

        let mut wait_sub_dirs = vec![(self, delta)];
        let mut wait_dir_count = 1;

        while wait_dir_count > 0 {
            let (base_dir, delta_dir) = wait_sub_dirs.remove(0);
            wait_dir_count -= 1;
            let mut append_dir_indexs = vec![];
            base_dir.attributes = delta_dir.attributes.clone();
            for child_index in 0..delta_dir.children.len() {
                let child = &delta_dir.children[child_index];
                let base_child_index = base_dir
                    .children
                    .iter()
                    .position(|base_child| base_child.name() == child.name());

                if let Some(base_child_index) = base_child_index {
                    let base_child = &base_dir.children[base_child_index];
                    let (is_remove_old, add_new_item, new_file_log) = match child {
                        StorageItem::Dir(dir, _) => {
                            let file_log = match base_child {
                                StorageItem::Dir(_, _) => {
                                    // compare later
                                    append_dir_indexs.push((base_child_index, child_index));
                                    wait_dir_count += 1;
                                    continue;
                                }
                                StorageItem::File(_, _) | StorageItem::FileDiff(_, _) => {
                                    // file removed log
                                    Some(None)
                                }
                                _ => None,
                            };
                            // type changed, remove it, and a new dir from delta will be added later.
                            (true, Some(StorageItem::Dir(dir.clone(), vec![])), file_log)
                        }
                        StorageItem::File(file, _) => {
                            // if it's a new file, replace it in base_dir, but the logs should be append it in base_dir.
                            // if it's same as base_dir, do nothing.
                            let mut is_new_file = true;
                            match base_child {
                                StorageItem::File(base_file, _) => {
                                    if base_file.hash == file.hash && base_file.size == file.size {
                                        is_new_file = false;
                                    }
                                }
                                StorageItem::FileDiff(base_diff, _) => {
                                    if base_diff.hash == file.hash && base_diff.size == file.size {
                                        is_new_file = false;
                                    }
                                }
                                _ => {}
                            }

                            // type changed, remove it, and a new file from delta will be added later.
                            let new_file_log = if is_new_file {
                                Some(Some(FileLog {
                                    hash: file.hash.clone(),
                                    size: file.size,
                                    service_meta: file.service_meta.clone(),
                                }))
                            } else {
                                None
                            };
                            (
                                true,
                                Some(StorageItem::File(file.clone(), vec![])),
                                new_file_log,
                            )
                        }
                        StorageItem::FileDiff(diff, _) => {
                            // similar to StorageItem::File
                            let mut is_new_file = true;
                            match base_child {
                                StorageItem::File(base_file, _) => {
                                    if base_file.hash == diff.hash && base_file.size == diff.size {
                                        is_new_file = false;
                                    }
                                }
                                StorageItem::FileDiff(base_diff, _) => {
                                    if base_diff.hash == diff.hash && base_diff.size == diff.size {
                                        is_new_file = false;
                                    }
                                }
                                _ => {}
                            }
                            // type changed, remove it, and a new file from delta will be added later.
                            let new_file_log = if is_new_file {
                                Some(Some(FileLog {
                                    hash: diff.hash.clone(),
                                    size: diff.size,
                                    service_meta: diff.service_meta.clone(),
                                }))
                            } else {
                                None
                            };

                            (
                                true,
                                Some(StorageItem::File(
                                    FileMeta {
                                        name: diff.name.clone(),
                                        attributes: diff.attributes.clone(),
                                        service_meta: diff.service_meta.clone(),
                                        hash: diff.hash.clone(),
                                        size: diff.size,
                                        upload_bytes: diff.upload_bytes,
                                    },
                                    vec![],
                                )),
                                new_file_log,
                            )
                        }
                        StorageItem::Link(link, _) => {
                            // if it's a new link, replace it in base_dir, but the logs should be append it in base_dir.
                            // if it's same as base_dir, do nothing.
                            let file_log = match base_child {
                                StorageItem::File(_, _) | StorageItem::FileDiff(_, _) => Some(None),
                                _ => None,
                            };

                            (
                                true,
                                Some(StorageItem::Link(link.clone(), vec![])),
                                file_log,
                            )
                        }
                        StorageItem::Log(log, _) => match log.action {
                            LogAction::Remove => {
                                let file_log = match base_child {
                                    StorageItem::File(_, _) | StorageItem::FileDiff(_, _) => {
                                        Some(None)
                                    }
                                    _ => None,
                                };

                                if base_child.is_file_in_history() || file_log.is_some() {
                                    (true, Some(StorageItem::Log(log.clone(), vec![])), file_log)
                                } else {
                                    (true, None, None)
                                }
                            }
                            LogAction::UpdateAttributes => {
                                let mut new_item = base_child.clone();
                                *new_item.attributes_mut() = log.attributes.clone();
                                (true, Some(new_item), None)
                            }
                        },
                    };

                    let mut file_logs = if is_remove_old {
                        // the index record in `append_dirs` is based on `base_dir.children `,
                        // so we need to -1 when remove it.
                        append_dir_indexs
                            .iter_mut()
                            .for_each(|(append_base_child_index, _)| {
                                if *append_base_child_index > base_child_index {
                                    *append_base_child_index -= 1;
                                }
                            });

                        base_dir.children.remove(base_child_index).take_logs()
                    } else {
                        vec![]
                    };

                    if let Some(new_file_log) = new_file_log {
                        file_logs.push(new_file_log);
                    }
                    if let Some(mut add_new_item) = add_new_item {
                        std::mem::swap(add_new_item.logs_mut(), &mut file_logs);
                        base_dir.children.push(add_new_item);
                    }
                } else {
                    // new item
                    base_dir.children.push(child.clone());
                }
            }

            if !append_dir_indexs.is_empty() {
                // sort append_dir_indexs by append_base_child_index asc
                append_dir_indexs
                    .sort_by_key(|(append_base_child_index, _)| *append_base_child_index);

                let mut wait_base_dirs = vec![];
                // splite base_dir.children at the indexs in `append_dir_indexs`
                let first_base_dir_index = append_dir_indexs[0].0;
                let (_, mut rest) = base_dir.children.split_at_mut(first_base_dir_index);
                for i in 1..append_dir_indexs.len() {
                    let (cut_count, _) = append_dir_indexs[i - 1];
                    let (append_base_child_index, _) = append_dir_indexs[i];
                    let (left, right) = rest.split_at_mut(append_base_child_index - cut_count);
                    rest = right;
                    wait_base_dirs.push(&mut left[0]);
                }
                wait_base_dirs.push(&mut rest[0]);

                for (_, delta_child_index) in append_dir_indexs {
                    let append_base_child = wait_base_dirs.remove(0);
                    let append_delta_child = &delta_dir.children[delta_child_index];
                    if let StorageItem::Dir(base_dir, _) = append_base_child {
                        if let StorageItem::Dir(delta_dir, _) = append_delta_child {
                            wait_sub_dirs.push((base_dir, delta_dir));
                        } else {
                            unreachable!("append_delta_child should be a directory");
                        }
                    } else {
                        unreachable!("append_delta_child should be a directory");
                    }
                }
            }
        }
    }

    pub fn find_file_service_meta(
        &self,
        hash: &str,
        file_size: u64,
        is_include_history: bool,
    ) -> Option<&Option<ServiceFileMetaType>> {
        let mut wait_search_dirs = vec![self];
        let mut next_dir_index = 0;

        while next_dir_index < wait_search_dirs.len() {
            let cur_dir = wait_search_dirs[next_dir_index];
            next_dir_index += 1;
            for child in cur_dir.children.iter() {
                match child {
                    StorageItem::Dir(dir, logs) => {
                        wait_search_dirs.push(dir);
                        if is_include_history {
                            if let Some(meta) = child.find_log(hash, file_size) {
                                return Some(&meta.service_meta);
                            }
                        }
                    }
                    StorageItem::File(file, _) => {
                        if file.hash == hash && file.size == file_size {
                            return Some(&file.service_meta);
                        }

                        if is_include_history {
                            if let Some(meta) = child.find_log(hash, file_size) {
                                return Some(&meta.service_meta);
                            }
                        }
                    }
                    StorageItem::FileDiff(diff, _) => {
                        if diff.hash == hash && diff.size == file_size {
                            return Some(&diff.service_meta);
                        }

                        if is_include_history {
                            if let Some(meta) = child.find_log(hash, file_size) {
                                return Some(&meta.service_meta);
                            }
                        }
                    }
                    StorageItem::Link(link, _) => {
                        if is_include_history {
                            if let Some(meta) = child.find_log(hash, file_size) {
                                return Some(&meta.service_meta);
                            }
                        }
                    }
                    StorageItem::Log(log, _) => {
                        if is_include_history {
                            if let Some(meta) = child.find_log(hash, file_size) {
                                return Some(&meta.service_meta);
                            }
                        }
                    }
                }
            }
        }

        None
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct FileMeta<ServiceFileMetaType: MetaBound> {
    pub name: PathBuf,
    pub attributes: StorageItemAttributes,
    #[serde(
        serialize_with = "serialize_meta_bound",
        deserialize_with = "deserialize_meta_bound"
    )]
    pub service_meta: Option<ServiceFileMetaType>,
    pub hash: String,
    pub size: u64,
    pub upload_bytes: u64,
}

impl<ServiceFileMetaType: MetaBound> StorageItemField for FileMeta<ServiceFileMetaType> {
    fn name(&self) -> &Path {
        &self.name
    }

    fn attributes(&self) -> &StorageItemAttributes {
        &self.attributes
    }

    fn attributes_mut(&mut self) -> &mut StorageItemAttributes {
        &mut self.attributes
    }
}

// It will work with a chunk
#[derive(Clone, Serialize, Deserialize)]
pub struct FileDiffChunk {
    pub pos: u64,           // position of the bytes stored in the chunk
    pub length: u64,        // length of the bytes
    pub origin_offset: u64, // the offset the bytes were instead in the original chunk
    pub origin_length: u64, // the length of the original bytes will be instead.
    pub upload_bytes: u64,
}

impl FileDiffChunk {
    // result = reader - base_reader
    pub async fn from_reader<'a>(
        base_reader: &mut FileStreamReader<'a>,
        reader: &mut FileStreamReader<'a>,
    ) -> BackupResult<Vec<FileDiffChunk>> {
        // TODO: diff

        let all_replace = FileDiffChunk {
            pos: 0,
            length: reader.file_size().await?,
            origin_offset: 0,
            origin_length: base_reader.file_size().await?,
            upload_bytes: 0,
        };

        Ok(vec![all_replace])
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct FileDiffMeta<ServiceDiffMetaType: MetaBound> {
    pub name: PathBuf,
    pub attributes: StorageItemAttributes,
    #[serde(
        serialize_with = "serialize_meta_bound",
        deserialize_with = "deserialize_meta_bound"
    )]
    pub service_meta: Option<ServiceDiffMetaType>,
    pub hash: String,
    pub size: u64,
    pub diff_chunks: Vec<FileDiffChunk>,
    pub upload_bytes: u64,
}

impl<ServiceDiffMetaType: MetaBound> StorageItemField for FileDiffMeta<ServiceDiffMetaType> {
    fn name(&self) -> &Path {
        &self.name
    }

    fn attributes(&self) -> &StorageItemAttributes {
        &self.attributes
    }

    fn attributes_mut(&mut self) -> &mut StorageItemAttributes {
        &mut self.attributes
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct LinkMeta<ServiceLinkMetaType: MetaBound> {
    pub name: PathBuf,
    pub attributes: StorageItemAttributes,
    #[serde(
        serialize_with = "serialize_meta_bound",
        deserialize_with = "deserialize_meta_bound"
    )]
    pub service_meta: Option<ServiceLinkMetaType>,
    pub target: PathBuf,
    pub is_hard: bool,
}

impl<ServiceLinkMetaType: MetaBound> StorageItemField for LinkMeta<ServiceLinkMetaType> {
    fn name(&self) -> &Path {
        &self.name
    }

    fn attributes(&self) -> &StorageItemAttributes {
        &self.attributes
    }

    fn attributes_mut(&mut self) -> &mut StorageItemAttributes {
        &mut self.attributes
    }
}

#[derive(Clone)]
pub enum LogAction {
    Remove,
    UpdateAttributes, // new attributes will be set in `attributes` field
}

impl Serialize for LogAction {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            LogAction::Remove => serializer.serialize_str("RM"),
            LogAction::UpdateAttributes => serializer.serialize_str("UA"),
        }
    }
}

impl<'de> Deserialize<'de> for LogAction {
    fn deserialize<D>(deserializer: D) -> Result<LogAction, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        if s == "RM" {
            Ok(LogAction::Remove)
        } else if s == "UA" {
            Ok(LogAction::UpdateAttributes)
        } else {
            Err(serde::de::Error::custom(format!(
                "Unknown log action: {}",
                s
            )))
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct LogMeta<ServiceLogMetaType: MetaBound> {
    pub name: PathBuf,
    pub attributes: StorageItemAttributes,
    #[serde(
        serialize_with = "serialize_meta_bound",
        deserialize_with = "deserialize_meta_bound"
    )]
    pub service_meta: Option<ServiceLogMetaType>,
    pub action: LogAction,
}

impl<ServiceLogMetaType: MetaBound> StorageItemField for LogMeta<ServiceLogMetaType> {
    fn name(&self) -> &Path {
        &self.name
    }

    fn attributes(&self) -> &StorageItemAttributes {
        &self.attributes
    }

    fn attributes_mut(&mut self) -> &mut StorageItemAttributes {
        &mut self.attributes
    }
}

pub trait StorageItemField {
    fn name(&self) -> &Path;
    fn attributes(&self) -> &StorageItemAttributes;
    fn attributes_mut(&mut self) -> &mut StorageItemAttributes;
}

pub type CheckPointMetaEngine = CheckPointMeta<String, String, String, String, String>;
pub type StorageItemEngine = StorageItem<String, String, String, String>;
pub type DirectoryMetaEngine = DirectoryMeta<String, String, String, String>;
pub type FileMetaEngine = FileMeta<String>;
pub type FileDiffMetaEngine = FileDiffMeta<String>;
pub type LinkMetaEngine = LinkMeta<String>;
pub type LogMetaEngine = LogMeta<String>;

// // meta for DMC, sample<TODO>.
// pub struct SectorId(u64);
// pub type ServiceDirMetaTypeDMC = ();
// pub type ServiceLinkMetaTypeDMC = ();
// pub type ServiceLogMetaTypeDMC = ();

// pub struct ChunkPositionDMC {
//     pub sector: SectorId,
//     pub pos: u64,    // Where this file storage in sector.
//     pub offset: u64, // If this file is too large, it will be cut into several chunks and stored in different sectors.
//     // the `offset` is the offset of the chunk in total file.
//     pub size: u64, //
// }

// pub struct ServiceMetaChunkTypeDMC {
//     pub chunks: Vec<ChunkPositionDMC>,
//     pub sector_count: u32, // Count of sectors occupied by this file; if the sector count is greater than it in `chunks` field, the file will be incomplete.
// }

// pub type ServiceFileMetaTypeDMC = ServiceMetaChunkTypeDMC;

// pub struct ServiceCheckPointMetaDMC {
//     pub sector_count: u32, // Count of sectors this checkpoint consumed. Some checkpoint will consume several sectors.
// }

fn serialize_system_time<S>(time: &SystemTime, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let duration = time
        .duration_since(UNIX_EPOCH)
        .map_err(serde::ser::Error::custom)?;
    serializer.serialize_u64(duration.as_secs())
}

fn deserialize_system_time<'de, D>(deserializer: D) -> Result<SystemTime, D::Error>
where
    D: Deserializer<'de>,
{
    let secs = u64::deserialize(deserializer)?;
    Ok(UNIX_EPOCH + std::time::Duration::from_secs(secs))
}

fn serialize_meta_bound<S, T>(v: &T, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
    T: MetaBound,
{
    let v_str = serde_json::to_string(v).map_err(serde::ser::Error::custom)?;
    serializer.serialize_str(v_str.as_str())
}

fn deserialize_meta_bound<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: MetaBound,
{
    let v_str = String::deserialize(deserializer)?;
    serde_json::from_str(&v_str).map_err(serde::de::Error::custom)
}
