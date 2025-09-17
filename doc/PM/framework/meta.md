# CheckPointMeta

```rust
// All times should use UTC time. Unless otherwise stated, they should be accurate to the second.

struct CheckPointMeta<ServiceItemMetaType, ServiceCheckPointMeta, ServiceDirMetaType, ServiceFileMetaType, ServiceLinkMetaType, ServiceLogMetaType> {
    task_friendly_name: String,
    task_uuid: String,
    version: CheckPointVersion,
    prev_versions: Vec<CheckPointVersion>, // all versions this checkpoint depends on
    create_time: SystemTime,
    complete_time: SystemTime,

    root: StorageItem<ServiceItemMetaType, ServiceDirMetaType, ServiceFileMetaType, ServiceLinkMetaType, ServiceLogMetaType>,

    // space size
    occupied_size: u64, // The really size the data described by this object occupied in the storage.
    consume_size: u64, // The size the data described by this object consumed in the storage, `consume_size` is greater than `occupied_size`, when the storage unit size is greater than 1 byte.
    all_prev_version_occupied_size: u64,
    all_prev_version_consume_size: u64,

    // Special for services
    service_meta: ServiceCheckPointMeta,
}

struct CheckPointVersion {
    time: SystemTime,
    seq: u64,
}

struct Attributes {
    create_time: SystemTime,
    last_update_time: SystemTime,
    owner: String,
    group: String,
    permissions: String,
}

// common meta
macro_rules! storage_item_common_meta {
    ($service_common_meta_type: ty) => {
        name: String,
        attributes: Attributes,
        parent: Option<Week<DirectoryMeta>>,
        service_meta: $service_common_meta_type, // any service can add meta here.
    }
}

enum StorageItem<ServiceDirMetaType, ServiceFileMetaType, ServiceLinkMetaType, ServiceLogMetaType> {
    Dir<DirectoryMeta<ServiceDirMetaType>>,
    File<FileMeta<ServiceFileMetaType>>,
    Link<LinkMeta<ServiceLinkMetaType>>,
    Log<LogMeta<ServiceLogMetaType>>
}

struct DirectoryMeta<ServiceDirMetaType, ServiceFileMetaType, ServiceLinkMetaType, ServiceLogMetaType> {
    storage_item_common_meta!(ServiceDirMetaType)
    children: Vec<StorageItem<ServiceDirMetaType, ServiceFileMetaType, ServiceLinkMetaType, ServiceLogMetaType>>,
}

struct FileMeta<ServiceFileMetaType> {
    storage_item_common_meta!(ServiceFileMetaType)
    hash: String,
    size: u64,
}

// It will work with a chunk
struct FileDiffChunk {
    pos: u64, // position of the bytes stored in the chunk
    length: u64, // length of the bytes
    origin_offset: u64, // the offset the bytes were instead in the original chunk
    origin_length: u64, // the length of the original bytes will be instead.
}

struct FileDiffMeta<ServiceDiffMetaType> {
    storage_item_common_meta!(ServiceDiffMetaType)
    hash: String,
    size: u64,
    diff_chunks: Vec<FileDiffChunk>,
}

struct LinkMeta<ServiceLinkMetaType> {
    storage_item_common_meta!(ServiceLinkMetaType)
    target: String,
    is_hard: bool,
}

enum LogAction {
    Remove,
    Recover,
    MoveFrom(String),
    MoveTo(String),
    CopyFrom(String),
    UpdateAttributes, // new attributes will be set in `attributes` field
}

struct LogMeta<ServiceLogMetaType> {
    storage_item_common_meta!(ServiceLogMetaType)
    action: LogAction,
}

// meta for DMC, sample.
type ServiceDirMetaTypeDMC = ();
type ServiceLinkMetaTypeDMC = ();
type ServiceLogMetaTypeDMC = ();

struct ChunkPositionDMC {
    sector: SectorId,
    pos: u64, // Where this file storage in sector.
    offset: u64, // If this file is too large, it will be cut into several chunks and stored in different sectors.
        // the `offset` is the offset of the chunk in total file.
    size: u64, //
}

struct ServiceMetaTypeDMC {
    chunks: Vec<ChunkPositionDMC>
    sector_count: u32, // Count of sectors occupied by this file; if the sector count is greater than it in `chunks` field, the file will be incomplete.
}

type ServiceFileMetaTypeDMC = ServiceMetaTypeDMC;

type ServiceDiffMetaTypeDMC = ServiceFileMetaTypeDMC;

struct ServiceCheckPointMetaDMC {
    sector_count: u32, // Count of sectors this checkpoint consumed. Some checkpoint will consume several sectors.
}
```
