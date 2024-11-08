use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::{
    checkpoint::DirChildType,
    engine::TaskUuid,
    error::{BackupError, BackupResult},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LockedSourceStateId(u64);

impl Into<u64> for LockedSourceStateId {
    fn into(self) -> u64 {
        self.0
    }
}

impl From<u64> for LockedSourceStateId {
    fn from(id: u64) -> Self {
        Self(id)
    }
}

// All times should use UTC time. Unless otherwise stated, they should be accurate to the second.

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
pub struct Attributes {
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

#[derive(Clone)]
pub enum FolderItem {
    Dir(DirectoryMeta),
    File(FileMeta),
    FileDiff(FileDiffMeta),
    Link(LinkMeta),
    Log(LogMeta),
}

impl Serialize for FolderItem {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Serialize `self` into the format `Type:Serialize::serialize(content)`
        match self {
            FolderItem::Dir(dir) => serializer
                .serialize_str(format!("DIR:{}", serde_json::to_string(dir).unwrap()).as_str()),
            FolderItem::File(file) => serializer
                .serialize_str(format!("FILE:{}", serde_json::to_string(file).unwrap()).as_str()),
            FolderItem::FileDiff(diff) => serializer
                .serialize_str(format!("DIFF:{}", serde_json::to_string(diff).unwrap()).as_str()),
            FolderItem::Link(link) => serializer
                .serialize_str(format!("LINK:{}", serde_json::to_string(link).unwrap()).as_str()),
            FolderItem::Log(log) => serializer
                .serialize_str(format!("LOG:{}", serde_json::to_string(log).unwrap()).as_str()),
        }
    }
}

impl<'de> Deserialize<'de> for FolderItem {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let v: Vec<&str> = s.splitn(2, ':').collect();
        if v.len() != 2 {
            return Err(serde::de::Error::custom(format!(
                "Unknown folder item: {}",
                s
            )));
        }
        match v[0] {
            "DIR" => {
                let dir: DirectoryMeta = serde_json::from_str(v[1]).unwrap();
                Ok(Self::Dir(dir))
            }
            "FILE" => {
                let file: FileMeta = serde_json::from_str(v[1]).unwrap();
                Ok(Self::File(file))
            }
            "DIFF" => {
                let diff: FileDiffMeta = serde_json::from_str(v[1]).unwrap();
                Ok(Self::FileDiff(diff))
            }
            "LINK" => {
                let link: LinkMeta = serde_json::from_str(v[1]).unwrap();
                Ok(Self::Link(link))
            }
            "LOG" => {
                let log: LogMeta = serde_json::from_str(v[1]).unwrap();
                Ok(Self::Log(log))
            }
            _ => Err(serde::de::Error::custom(format!(
                "Unknown storage item: {}",
                s
            ))),
        }
    }
}

impl FolderItemField for FolderItem {
    fn path(&self) -> &Path {
        match self {
            FolderItem::Dir(d) => d.path(),
            FolderItem::File(f) => f.path(),
            FolderItem::FileDiff(d) => d.path(),
            FolderItem::Link(l) => l.path(),
            FolderItem::Log(l) => l.path(),
        }
    }

    fn attributes(&self) -> &Attributes {
        match self {
            FolderItem::Dir(d) => d.attributes(),
            FolderItem::File(f) => f.attributes(),
            FolderItem::FileDiff(d) => d.attributes(),
            FolderItem::Link(l) => l.attributes(),
            FolderItem::Log(l) => l.attributes(),
        }
    }

    fn attributes_mut(&mut self) -> &mut Attributes {
        match self {
            FolderItem::Dir(d) => d.attributes_mut(),
            FolderItem::File(f) => f.attributes_mut(),
            FolderItem::FileDiff(d) => d.attributes_mut(),
            FolderItem::Link(l) => l.attributes_mut(),
            FolderItem::Log(l) => l.attributes_mut(),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct DirectoryMeta {
    pub path: PathBuf,
    pub attributes: Attributes,
}

impl FolderItemField for DirectoryMeta {
    fn path(&self) -> &Path {
        &self.path
    }

    fn attributes(&self) -> &Attributes {
        &self.attributes
    }

    fn attributes_mut(&mut self) -> &mut Attributes {
        &mut self.attributes
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct FileMeta {
    pub path: PathBuf,
    pub attributes: Attributes,
    pub hash: String,
    pub size: u64,
}

impl FolderItemField for FileMeta {
    fn path(&self) -> &Path {
        &self.path
    }

    fn attributes(&self) -> &Attributes {
        &self.attributes
    }

    fn attributes_mut(&mut self) -> &mut Attributes {
        &mut self.attributes
    }
}

pub enum NewDiffBlockPos {
    NewFile(u64),
    DiffFile(u64),
    Both(u64, u64), // <new-file-pos, diff-file-pos>
}

impl NewDiffBlockPos {
    pub fn new_file_pos(&self) -> Option<u64> {
        match self {
            NewDiffBlockPos::NewFile(pos) => Some(pos),
            NewDiffBlockPos::DiffFile(_) => None,
            NewDiffBlockPos::Both(pos, _) => Some(pos),
        }
    }

    pub fn diff_file_pos(&self) -> Option<u64> {
        match self {
            NewDiffBlockPos::NewFile(_) => None,
            NewDiffBlockPos::DiffFile(pos) => Some(pos),
            NewDiffBlockPos::Both(_, pos) => Some(pos),
        }
    }
}

pub struct DefaultFileDiffBlock {
    original_offset: u64,
    original_len: u64,
    new_block_pos: NewDiffBlockPos,
    new_len: u64,
}

#[derive(Clone)]
pub struct DefaultFileDiffHeader {
    pub blocks: Vec<DefaultFileDiffBlock>,
}

#[derive(Clone)]
pub struct CustomFileDiffHeader {
    pub classify: String,
    pub param: Vec<u8>,
}

#[derive(Clone)]
pub enum FileDiffHeader {
    Default,
    Custom(CustomFileDiffHeader),
}

#[derive(Clone, Serialize, Deserialize)]
pub struct FileDiffMeta {
    pub path: PathBuf,
    pub attributes: Attributes,
    pub file_size: u64,
    pub file_hash: Option<String>,
    pub hash: String,
    pub size: u64,
    pub header: FileDiffHeader,
}

impl FolderItemField for FileDiffMeta {
    fn path(&self) -> &Path {
        &self.path
    }

    fn attributes(&self) -> &Attributes {
        &self.attributes
    }

    fn attributes_mut(&mut self) -> &mut Attributes {
        &mut self.attributes
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct LinkMeta {
    pub path: PathBuf,
    pub attributes: Attributes,
    pub target: PathBuf,
    pub is_hard: bool,
}

impl FolderItemField for LinkMeta {
    fn path(&self) -> &Path {
        &self.path
    }

    fn attributes(&self) -> &Attributes {
        &self.attributes
    }

    fn attributes_mut(&mut self) -> &mut Attributes {
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
pub struct LogMeta {
    pub path: PathBuf,
    pub attributes: Attributes,
    pub action: LogAction,
}

impl FolderItemField for LogMeta {
    fn path(&self) -> &Path {
        &self.path
    }

    fn attributes(&self) -> &Attributes {
        &self.attributes
    }

    fn attributes_mut(&mut self) -> &mut Attributes {
        &mut self.attributes
    }
}

pub trait FolderItemField {
    fn path(&self) -> &Path;
    fn attributes(&self) -> &Attributes;
    fn attributes_mut(&mut self) -> &mut Attributes;
}

pub struct ChunkId(u64);

pub struct ChunkInfo {
    pub id: ChunkId,
    pub hash: Option<String>,
    pub size: u64,
    pub compress: Option<String>,
}

pub enum ChunkBlockMeta {
    File(FileMeta),
    Block(BlockMeta),
    FileDiff(FileDiffMeta),
}

pub struct ChunkBlock {
    meta: ChunkBlockMeta,
    pos: u64,
    compress_size: Option<u64>,
}

pub enum ChunkItem {
    Dir(DirectoryMeta),
    Link(LinkMeta),
    Log(LogMeta),
    Block(ChunkBlock),
}

#[derive(Clone, Serialize, Deserialize)]
pub struct BlockMeta {
    pub path: PathBuf,
    pub attributes: Option<Attributes>,
    pub file_size: u64,
    pub file_hash: Option<String>,
    pub hash: String,
    pub offset: u64,
    pub len: u64,
}

pub trait ChunkItemField {
    fn path(&self) -> &Path;
    fn attributes(&self) -> Option<&Attributes>;
    fn attributes_mut(&mut self) -> Option<&mut Attributes>;
}

impl ChunkItemField for BlockMeta {
    fn path(&self) -> &Path {
        &self.path
    }

    fn attributes(&self) -> Option<&Attributes> {
        self.attributes.as_ref()
    }

    fn attributes_mut(&mut self) -> Option<&mut Attributes> {
        self.attributes.as_mut()
    }
}

impl<T> ChunkItemField for T
where
    T: FolderItemField,
{
    fn path(&self) -> &Path {
        self.path::<FolderItemField>()
    }

    fn attributes(&self) -> Option<&Attributes> {
        Some(self.attributes::<FolderItemField>())
    }

    fn attributes_mut(&mut self) -> Option<&mut Attributes> {
        Some(self.attributes_mut::<FolderItemField>())
    }
}

impl ChunkItemField for ChunkBlockMeta {
    fn path(&self) -> &Path {
        match self {
            ChunkBlockMeta::File(file_meta) => file_meta.path::<ChunkItemField>(),
            ChunkBlockMeta::Block(block_meta) => block_meta.path::<ChunkItemField>(),
            ChunkBlockMeta::FileDiff(file_diff_meta) => file_diff_meta.path::<ChunkItemField>(),
        }
    }

    fn attributes(&self) -> Option<&Attributes> {
        let attr = match self {
            ChunkBlockMeta::File(file_meta) => file_meta.attributes::<ChunkItemField>(),
            ChunkBlockMeta::Block(block_meta) => return block_meta.attributes::<ChunkItemField>(),
            ChunkBlockMeta::FileDiff(file_diff_meta) => {
                file_diff_meta.attributes::<ChunkItemField>()
            }
        };
        Some(attr)
    }

    fn attributes_mut(&mut self) -> Option<&mut Attributes> {
        let attr = match self {
            ChunkBlockMeta::File(file_meta) => file_meta.attributes_mut::<ChunkItemField>(),
            ChunkBlockMeta::Block(block_meta) => {
                return block_meta.attributes_mut::<ChunkItemField>()
            }
            ChunkBlockMeta::FileDiff(file_diff_meta) => {
                file_diff_meta.attributes_mut::<ChunkItemField>()
            }
        };
        Some(attr)
    }
}

impl ChunkItemField for ChunkItem {
    fn path(&self) -> &Path {
        match self {
            ChunkItem::Dir(directory_meta) => directory_meta.path(),
            ChunkItem::Link(link_meta) => link_meta.path(),
            ChunkItem::Log(log_meta) => log_meta.path(),
            ChunkItem::Block(chunk_block) => chunk_block.path(),
        }
    }

    fn attributes(&self) -> Option<&Attributes> {
        let attr = match self {
            ChunkItem::Dir(directory_meta) => directory_meta.attributes(),
            ChunkItem::Link(link_meta) => link_meta.attributes(),
            ChunkItem::Log(log_meta) => log_meta.attributes(),
            ChunkItem::Block(chunk_block) => return chunk_block.attributes(),
        };
        Some(attr)
    }

    fn attributes_mut(&mut self) -> Option<&mut Attributes> {
        let attr = match self {
            ChunkItem::Dir(directory_meta) => directory_meta.attributes_mut(),
            ChunkItem::Link(link_meta) => link_meta.attributes_mut(),
            ChunkItem::Log(log_meta) => log_meta.attributes_mut(),
            ChunkItem::Block(chunk_block) => return chunk_block.attributes_mut(),
        };
        Some(attr)
    }
}

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
