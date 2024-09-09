use std::time::SystemTime;

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

pub trait MetaBound: Clone {}

impl<T> MetaBound for T where T: Clone {}

#[derive(Clone)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CheckPointVersion {
    pub time: SystemTime,
    pub seq: u64,
}

#[derive(Clone)]
pub struct StorageItemAttributes {
    pub create_time: SystemTime,
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

#[derive(Clone)]
pub struct DirectoryMeta<
    ServiceDirMetaType: MetaBound,
    ServiceFileMetaType: MetaBound,
    ServiceLinkMetaType: MetaBound,
    ServiceLogMetaType: MetaBound,
> {
    pub name: Vec<u8>,
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

#[derive(Clone)]
pub struct FileMeta<ServiceFileMetaType: MetaBound> {
    pub name: Vec<u8>,
    pub attributes: StorageItemAttributes,
    pub service_meta: Option<ServiceFileMetaType>,
    pub hash: String,
    pub size: u64,
    pub upload_bytes: u64,
}

// It will work with a chunk
#[derive(Clone)]
pub struct FileDiffChunk {
    pub pos: u64,           // position of the bytes stored in the chunk
    pub length: u64,        // length of the bytes
    pub origin_offset: u64, // the offset the bytes were instead in the original chunk
    pub origin_length: u64, // the length of the original bytes will be instead.
    pub upload_bytes: u64,
}

#[derive(Clone)]
pub struct FileDiffMeta<ServiceDiffMetaType: MetaBound> {
    pub name: Vec<u8>,
    pub attributes: StorageItemAttributes,
    pub service_meta: Option<ServiceDiffMetaType>,
    pub hash: String,
    pub size: u64,
    pub diff_chunks: Vec<FileDiffChunk>,
}

#[derive(Clone)]
pub struct LinkMeta<ServiceLinkMetaType: MetaBound> {
    pub name: Vec<u8>,
    pub attributes: StorageItemAttributes,
    pub service_meta: Option<ServiceLinkMetaType>,
    pub target: String,
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

#[derive(Clone)]
pub struct LogMeta<ServiceLogMetaType: MetaBound> {
    pub name: Vec<u8>,
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
