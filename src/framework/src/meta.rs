use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{de::DeserializeOwned, Deserialize, Deserializer, Serialize, Serializer};

use crate::{
    checkpoint::{DirChildType, FileStreamReader, StorageReader},
    engine::TaskUuid,
    error::BackupResult,
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
    ServiceDiffMetaType: MetaBound,
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
        ServiceDiffMetaType,
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
        ServiceDiffMetaType: MetaBound,
        ServiceLinkMetaType: MetaBound,
        ServiceLogMetaType: MetaBound,
    >
    CheckPointMeta<
        ServiceCheckPointMeta,
        ServiceDirMetaType,
        ServiceFileMetaType,
        ServiceDiffMetaType,
        ServiceLinkMetaType,
        ServiceLogMetaType,
    >
{
    pub fn estimate_occupy_size(&self) -> u64 {
        // meta size
        let mut occupy_size = serde_json::to_string(self).unwrap().len() as u64;
        // calculate the size of all files(Type: DirChildType::File) in the meta.
        // Traverse it as meta_from_reader

        let mut wait_read_dirs = vec![&self.root];

        while !wait_read_dirs.is_empty() {
            let current_item = wait_read_dirs.remove(0);

            match current_item {
                StorageItem::Dir(dir) => {
                    for child in dir.children.iter() {
                        match child {
                            StorageItem::Dir(_) => {
                                wait_read_dirs.push(child);
                            }
                            StorageItem::File(file) => {
                                occupy_size += file.size;
                            }
                            StorageItem::Link(_) => {}
                            StorageItem::Log(_) => {}
                            StorageItem::FileDiff(diff) => {
                                occupy_size +=
                                    diff.diff_chunks.iter().map(|diff| diff.length).sum::<u64>();
                            }
                        }
                    }
                }
                StorageItem::File(file) => {
                    occupy_size += file.size;
                }
                StorageItem::Link(_) => {}
                StorageItem::Log(_) => {}
                StorageItem::FileDiff(diff) => {
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

// common meta
#[derive(Clone)]
pub enum StorageItem<
    ServiceDirMetaType: MetaBound,
    ServiceFileMetaType: MetaBound,
    ServiceDiffMetaType: MetaBound,
    ServiceLinkMetaType: MetaBound,
    ServiceLogMetaType: MetaBound,
> {
    Dir(
        DirectoryMeta<
            ServiceDirMetaType,
            ServiceFileMetaType,
            ServiceDiffMetaType,
            ServiceLinkMetaType,
            ServiceLogMetaType,
        >,
    ),
    File(FileMeta<ServiceFileMetaType>),
    FileDiff(FileDiffMeta<ServiceDiffMetaType>),
    Link(LinkMeta<ServiceLinkMetaType>),
    Log(LogMeta<ServiceLogMetaType>),
}

impl<
        ServiceDirMetaType: MetaBound,
        ServiceFileMetaType: MetaBound,
        ServiceDiffMetaType: MetaBound,
        ServiceLinkMetaType: MetaBound,
        ServiceLogMetaType: MetaBound,
    > Serialize
    for StorageItem<
        ServiceDirMetaType,
        ServiceFileMetaType,
        ServiceDiffMetaType,
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
            StorageItem::Dir(dir) => serializer
                .serialize_str(format!("DIR:{}", serde_json::to_string(dir).unwrap()).as_str()),
            StorageItem::File(file) => serializer
                .serialize_str(format!("FILE:{}", serde_json::to_string(file).unwrap()).as_str()),
            StorageItem::Link(link) => serializer
                .serialize_str(format!("LINK:{}", serde_json::to_string(link).unwrap()).as_str()),
            StorageItem::Log(log) => serializer
                .serialize_str(format!("LOG:{}", serde_json::to_string(log).unwrap()).as_str()),
            StorageItem::FileDiff(diff) => serializer
                .serialize_str(format!("DIFF:{}", serde_json::to_string(diff).unwrap()).as_str()),
        }
    }
}

impl<
        'de,
        ServiceDirMetaType: MetaBound,
        ServiceFileMetaType: MetaBound,
        ServiceDiffMetaType: MetaBound,
        ServiceLinkMetaType: MetaBound,
        ServiceLogMetaType: MetaBound,
    > Deserialize<'de>
    for StorageItem<
        ServiceDirMetaType,
        ServiceFileMetaType,
        ServiceDiffMetaType,
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
            ServiceDiffMetaType,
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
                    ServiceDiffMetaType,
                    ServiceLinkMetaType,
                    ServiceLogMetaType,
                > = serde_json::from_str(v[1]).unwrap();
                Ok(StorageItem::Dir(dir))
            }
            "FILE" => {
                let file: FileMeta<ServiceFileMetaType> = serde_json::from_str(v[1]).unwrap();
                Ok(StorageItem::File(file))
            }
            "LINK" => {
                let link: LinkMeta<ServiceLinkMetaType> = serde_json::from_str(v[1]).unwrap();
                Ok(StorageItem::Link(link))
            }
            "LOG" => {
                let log: LogMeta<ServiceLogMetaType> = serde_json::from_str(v[1]).unwrap();
                Ok(StorageItem::Log(log))
            }
            "DIFF" => {
                let diff: FileDiffMeta<ServiceDiffMetaType> = serde_json::from_str(v[1]).unwrap();
                Ok(StorageItem::FileDiff(diff))
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
        ServiceDiffMetaType: MetaBound,
        ServiceLinkMetaType: MetaBound,
        ServiceLogMetaType: MetaBound,
    > StorageItemField
    for StorageItem<
        ServiceDirMetaType,
        ServiceFileMetaType,
        ServiceDiffMetaType,
        ServiceLinkMetaType,
        ServiceLogMetaType,
    >
{
    fn name(&self) -> &Path {
        match self {
            StorageItem::Dir(d) => d.name(),
            StorageItem::File(f) => f.name(),
            StorageItem::Link(l) => l.name(),
            StorageItem::Log(l) => l.name(),
            StorageItem::FileDiff(d) => d.name(),
        }
    }

    fn attributes(&self) -> &StorageItemAttributes {
        match self {
            StorageItem::Dir(d) => d.attributes(),
            StorageItem::File(f) => f.attributes(),
            StorageItem::Link(l) => l.attributes(),
            StorageItem::Log(l) => l.attributes(),
            StorageItem::FileDiff(d) => d.attributes(),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct DirectoryMeta<
    ServiceDirMetaType: MetaBound,
    ServiceFileMetaType: MetaBound,
    ServiceDiffMetaType: MetaBound,
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
            ServiceDiffMetaType,
            ServiceLinkMetaType,
            ServiceLogMetaType,
        >,
    >,
}

impl<
        ServiceDirMetaType: MetaBound,
        ServiceFileMetaType: MetaBound,
        ServiceDiffMetaType: MetaBound,
        ServiceLinkMetaType: MetaBound,
        ServiceLogMetaType: MetaBound,
    > StorageItemField
    for DirectoryMeta<
        ServiceDirMetaType,
        ServiceFileMetaType,
        ServiceDiffMetaType,
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
}

impl<
        ServiceDirMetaType: MetaBound,
        ServiceFileMetaType: MetaBound,
        ServiceDiffMetaType: MetaBound,
        ServiceLinkMetaType: MetaBound,
        ServiceLogMetaType: MetaBound,
    >
    DirectoryMeta<
        ServiceDirMetaType,
        ServiceFileMetaType,
        ServiceDiffMetaType,
        ServiceLinkMetaType,
        ServiceLogMetaType,
    >
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

        let mut all_read_files = vec![(None, StorageItem::Dir(root_dir), root_name)]; // <parent-index, item, full-path>
        let mut next_dir_index = 0;

        while next_dir_index < all_read_files.len() {
            let parent_index = next_dir_index;
            let parent_full_path = all_read_files[next_dir_index].2.as_path();
            let current_item = &all_read_files[next_dir_index].1;
            next_dir_index += 1;
            if let StorageItem::Dir(_) = current_item {
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
                        StorageItem::File(FileMeta {
                            name: child_name,
                            attributes: child_attr,
                            service_meta: None,
                            hash,
                            size: file_size,
                            upload_bytes: 0,
                        })
                    }
                    DirChildType::Dir(_) => StorageItem::Dir(DirectoryMeta {
                        name: child_name,
                        attributes: child_attr,
                        service_meta: None,
                        children: vec![],
                    }),
                    DirChildType::Link(_) => {
                        let link_info = reader.read_link(&child_name).await?;
                        StorageItem::Link(LinkMeta {
                            name: child_name,
                            attributes: child_attr,
                            service_meta: None,
                            is_hard: link_info.is_hard,
                            target: link_info.target,
                        })
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
                if let StorageItem::Dir(parent_dir) = parent {
                    parent_dir.children.push(child_meta);
                } else {
                    unreachable!("only directory can have children");
                }
            } else if let StorageItem::Dir(child_meta) = child_meta {
                return Ok(child_meta);
            }
        }
    }

    // meta = meta(reader) - meta(base_meta)
    pub async fn from_delta(
        base_meta: &StorageItem<
            ServiceDirMetaType,
            ServiceFileMetaType,
            ServiceDiffMetaType,
            ServiceLinkMetaType,
            ServiceLogMetaType,
        >,
        base_reader: &dyn StorageReader,
        reader: &dyn StorageReader,
    ) -> BackupResult<Self> {
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
        let base_meta = if let StorageItem::Dir(base_dir) = base_meta {
            base_dir
        } else {
            base_root_dir.children.push(base_meta.clone());
            &base_root_dir
        };

        let mut all_read_files = vec![(None, StorageItem::Dir(root_dir), root_name)]; // <parent-index, item, full-path>
        let mut all_reader_dir_indexs = vec![(0, true)]; // <index-in-all_read_files, is-updated>
        let mut all_base_dirs = vec![Some(base_meta)];
        let mut next_dir_index = 0;

        while next_dir_index < all_reader_dir_indexs.len() {
            let current_base_dir = all_base_dirs[next_dir_index];
            let (parent_index, is_dir_attr_update) = all_reader_dir_indexs[next_dir_index];
            let parent_full_path = all_read_files[parent_index].2.clone();
            let current_item = &all_read_files[parent_index].1;
            next_dir_index += 1;
            let current_dir = if let StorageItem::Dir(current_dir) = current_item {
                current_dir
            } else {
                unreachable!("only directory can have children");
            };

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
                        match base_child {
                            Some(base_child) => {
                                // TODO: move/copy/recovery from other directory.
                                if let StorageItem::File(base_file) = base_child {
                                    if base_file.hash == hash && base_file.size == file_size {
                                        // no update
                                        None
                                    } else {
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
                                        Some(StorageItem::FileDiff(FileDiffMeta {
                                            name: child_name,
                                            attributes: child_attr,
                                            service_meta: None,
                                            hash,
                                            size: file_size,
                                            upload_bytes: 0,
                                            diff_chunks: diffs,
                                        }))
                                    }
                                } else {
                                    // new file
                                    Some(StorageItem::File(FileMeta {
                                        name: child_name,
                                        attributes: child_attr,
                                        service_meta: None,
                                        hash,
                                        size: file_size,
                                        upload_bytes: 0,
                                    }))
                                }
                            }
                            None => {
                                // new file
                                Some(StorageItem::File(FileMeta {
                                    name: child_name,
                                    attributes: child_attr,
                                    service_meta: None,
                                    hash,
                                    size: file_size,
                                    upload_bytes: 0,
                                }))
                            }
                        }
                    }
                    DirChildType::Dir(_) => {
                        let is_type_changed = match base_child {
                            Some(base_child) => {
                                if let StorageItem::Dir(base_child) = base_child {
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
                        Some(StorageItem::Dir(DirectoryMeta {
                            name: child_name,
                            attributes: child_attr,
                            service_meta: None,
                            children: vec![],
                        }))
                    }
                    DirChildType::Link(_) => {
                        let link_info = reader.read_link(&child_name).await?;
                        let mut is_updated = true;
                        if let Some(base_child) = base_child {
                            if let StorageItem::Link(base_link) = base_child {
                                if base_link.is_hard == link_info.is_hard
                                    && base_link.target == link_info.target
                                {
                                    // no update
                                    is_updated = false;
                                }
                            }
                        }

                        if is_updated {
                            Some(StorageItem::Link(LinkMeta {
                                name: child_name,
                                attributes: child_attr,
                                service_meta: None,
                                is_hard: link_info.is_hard,
                                target: link_info.target,
                            }))
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
                        StorageItem::Log(update_attr_log),
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
                            StorageItem::Log(LogMeta {
                                name: child.name().to_path_buf(),
                                attributes: child.attributes().clone(),
                                service_meta: None,
                                action: LogAction::Remove,
                            }),
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
                if let StorageItem::Dir(parent_dir) = parent {
                    parent_dir.children.push(child_meta);
                } else {
                    unreachable!("only directory can have children");
                }
            } else if let StorageItem::Dir(child_meta) = child_meta {
                return Ok(child_meta);
            }
        }
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
}

#[derive(Clone)]
pub enum LogAction {
    Remove,
    Recover,
    CopyFrom(String),
    UpdateAttributes, // new attributes will be set in `attributes` field
}

impl Serialize for LogAction {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            LogAction::Remove => serializer.serialize_str("RM"),
            LogAction::Recover => serializer.serialize_str("RC"),
            LogAction::CopyFrom(from) => serializer.serialize_str(format!("CF:{}", from).as_str()),
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
        } else if s == "RC" {
            Ok(LogAction::Recover)
        } else if s.starts_with("CF:") {
            Ok(LogAction::CopyFrom(s[3..].to_string()))
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
}

pub trait StorageItemField {
    fn name(&self) -> &Path;
    fn attributes(&self) -> &StorageItemAttributes;
}

pub type CheckPointMetaEngine = CheckPointMeta<String, String, String, String, String, String>;
pub type StorageItemEngine = StorageItem<String, String, String, String, String>;
pub type DirectoryMetaEngine = DirectoryMeta<String, String, String, String, String>;
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

// pub type ServiceDiffMetaTypeDMC = ServiceMetaChunkTypeDMC;

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
    v.serialize(serializer)
}

fn deserialize_meta_bound<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: MetaBound,
{
    let v = T::deserialize(deserializer)?;
    Ok(v)
}
