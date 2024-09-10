use std::{
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{de::DeserializeOwned, Deserialize, Deserializer, Serialize, Serializer};

use crate::engine::TaskUuid;

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
    pub service_meta: Option<ServiceCheckPointMeta>,
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

#[derive(Clone, Serialize, Deserialize)]
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
    ),
    File(FileMeta<ServiceFileMetaType>),
    Link(LinkMeta<ServiceLinkMetaType>),
    Log(LogMeta<ServiceLogMetaType>),
}

impl <
    ServiceDirMetaType: MetaBound,
    ServiceFileMetaType: MetaBound,
    ServiceLinkMetaType: MetaBound,
    ServiceLogMetaType: MetaBound,
> Serialize for StorageItem<
    ServiceDirMetaType,
    ServiceFileMetaType,
    ServiceLinkMetaType,
    ServiceLogMetaType,
> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Serialize `self` into the format `Type:Serialize::serialize(content)`
        match self {
            StorageItem::Dir(dir) => {
                serializer.serialize_str(format!("DIR:{}", serde_json::to_string(dir).unwrap()).as_str())
            }
            StorageItem::File(file) => {
                serializer.serialize_str(format!("FILE:{}", serde_json::to_string(file).unwrap()).as_str())
            }
            StorageItem::Link(link) => {
                serializer.serialize_str(format!("LINK:{}", serde_json::to_string(link).unwrap()).as_str())
            }
            StorageItem::Log(log) => {
                serializer.serialize_str(format!("LOG:{}", serde_json::to_string(log).unwrap()).as_str())
            }
        }
    }
}

impl <'de,
    ServiceDirMetaType: MetaBound,
    ServiceFileMetaType: MetaBound,
    ServiceLinkMetaType: MetaBound,
    ServiceLogMetaType: MetaBound
> Deserialize<'de> for StorageItem<
    ServiceDirMetaType,
    ServiceFileMetaType,
    ServiceLinkMetaType,
    ServiceLogMetaType,
> {
    fn deserialize<D>(deserializer: D) -> Result<StorageItem<
        ServiceDirMetaType,
        ServiceFileMetaType,
        ServiceLinkMetaType,
        ServiceLogMetaType,
    >, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let v: Vec<&str> = s.splitn(2, ':').collect();
        if v.len() != 2 {
            return Err(serde::de::Error::custom(format!("Unknown storage item: {}", s)));
        }
        match v[0] {
            "DIR" => {
                let dir: DirectoryMeta<
                    ServiceDirMetaType,
                    ServiceFileMetaType,
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
            _ => Err(serde::de::Error::custom(format!("Unknown storage item: {}", s))),
        }
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
    pub service_meta: Option<ServiceDirMetaType>,
    pub children: Vec<
        StorageItem<
            ServiceDirMetaType,
            ServiceFileMetaType,
            ServiceLinkMetaType,
            ServiceLogMetaType,
        >,
    >,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct FileMeta<ServiceFileMetaType: MetaBound> {
    pub name: PathBuf,
    pub attributes: StorageItemAttributes,
    pub service_meta: Option<ServiceFileMetaType>,
    pub hash: String,
    pub size: u64,
    pub upload_bytes: u64,
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

#[derive(Clone, Serialize, Deserialize)]
pub struct FileDiffMeta<ServiceDiffMetaType: MetaBound> {
    pub name: PathBuf,
    pub attributes: StorageItemAttributes,
    pub service_meta: Option<ServiceDiffMetaType>,
    pub hash: String,
    pub size: u64,
    pub diff_chunks: Vec<FileDiffChunk>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct LinkMeta<ServiceLinkMetaType: MetaBound> {
    pub name: PathBuf,
    pub attributes: StorageItemAttributes,
    pub service_meta: Option<ServiceLinkMetaType>,
    pub target: PathBuf,
    pub is_hard: bool,
}

#[derive(Clone)]
pub enum LogAction {
    Remove,
    Recover,
    MoveFrom(String),
    MoveTo(String),
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
            LogAction::MoveFrom(from) => serializer.serialize_str(format!("MF:{}", from).as_str()),
            LogAction::MoveTo(to) => serializer.serialize_str(format!("MT:{}", to).as_str())
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
        } else if s.starts_with("MF:") {
            Ok(LogAction::MoveFrom(s[3..].to_string()))
        } else if s.starts_with("MT:") {
            Ok(LogAction::MoveTo(s[3..].to_string()))
        } else if s.starts_with("CF:") {
            Ok(LogAction::CopyFrom(s[3..].to_string()))
        } else if s == "UA" {
            Ok(LogAction::UpdateAttributes)
        } else {
            Err(serde::de::Error::custom(format!("Unknown log action: {}", s)))
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct LogMeta<ServiceLogMetaType: MetaBound> {
    pub name: PathBuf,
    pub attributes: StorageItemAttributes,
    pub service_meta: Option<ServiceLogMetaType>,
    pub action: LogAction,
}

pub type CheckPointMetaEngine = CheckPointMeta<String, String, String, String, String>;
pub type StorageItemEngine = StorageItem<String, String, String, String>;
pub type DirectoryMetaEngine = DirectoryMeta<String, String, String, String>;
pub type FileMetaEngine = FileMeta<String>;
pub type FileDiffMetaEngine = FileDiffMeta<String>;
pub type LinkMetaEngine = LinkMeta<String>;
pub type LogMetaEngine = LogMeta<String>;

// meta for DMC, sample<TODO>.
pub struct SectorId(u64);
pub type ServiceDirMetaTypeDMC = ();
pub type ServiceLinkMetaTypeDMC = ();
pub type ServiceLogMetaTypeDMC = ();

pub struct ChunkPositionDMC {
    pub sector: SectorId,
    pub pos: u64,    // Where this file storage in sector.
    pub offset: u64, // If this file is too large, it will be cut into several chunks and stored in different sectors.
    // the `offset` is the offset of the chunk in total file.
    pub size: u64, //
}

pub struct ServiceMetaChunkTypeDMC {
    pub chunks: Vec<ChunkPositionDMC>,
    pub sector_count: u32, // Count of sectors occupied by this file; if the sector count is greater than it in `chunks` field, the file will be incomplete.
}

pub type ServiceFileMetaTypeDMC = ServiceMetaChunkTypeDMC;

pub type ServiceDiffMetaTypeDMC = ServiceMetaChunkTypeDMC;

pub struct ServiceCheckPointMetaDMC {
    pub sector_count: u32, // Count of sectors this checkpoint consumed. Some checkpoint will consume several sectors.
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
