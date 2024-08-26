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

struct StorageItemAttributes {
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
        attributes: StorageItemAttributes,
        parent: Option<Week<DirectoryMeta>>,
        service_meta: $service_common_meta_type, // any service can add meta here.
    }
}

enum StorageItem<ServiceMetaType, ServiceDirMetaType, ServiceFileMetaType, ServiceLinkMetaType, ServiceLogMetaType> {
    Dir<DirectoryMeta<ServiceMetaType, ServiceDirMetaType>>,
    File<FileMeta<ServiceMetaType, ServiceFileMetaType>>,
    Link<LinkMeta<ServiceMetaType, ServiceLinkMetaType>>,
    Log<LogMeta<ServiceMetaType, ServiceLogMetaType>>
}

struct DirectoryMeta<ServiceMetaType, ServiceDirMetaType, ServiceFileMetaType, ServiceLinkMetaType, ServiceLogMetaType> {
    storage_item_common_meta!(ServiceMetaType)
    children: Vec<StorageItem<ServiceMetaType, ServiceDirMetaType, ServiceFileMetaType, ServiceLinkMetaType, ServiceLogMetaType>>,
    service_meta: ServiceDirMetaType,
}

struct FileMeta<ServiceMetaType, ServiceFileMetaType> {
    storage_item_common_meta!(ServiceMetaType)
    hash: String,
    size: u64,
    service_meta: ServiceFileMetaType,
}

// It will work with a chunk
struct FileDiffChunk {
    pos: u64, // position of the bytes stored in the chunk
    length: u64, // length of the bytes
    origin_offset: u64, // the offset the bytes were instead in the original chunk
    origin_length: u64, // the length of the original bytes will be instead.
}

struct FileDiffMeta<ServiceMetaType, ServiceDiffMetaType> {
    storage_item_common_meta!(ServiceMetaType)
    hash: String,
    size: u64,
    diff_chunks: Vec<FileDiffChunk>,
    service_meta: ServiceDiffMetaType,
}

struct LinkMeta<ServiceMetaType, ServiceLinkMetaType> {
    storage_item_common_meta!(ServiceMetaType)
    target: String,
    is_hard: bool,
    service_meta: ServiceLinkMetaType,
}

enum LogAction {
    Remove,
    Recover,
    MoveFrom(String),
    MoveTo(String),
    CopyFrom(String),
    UpdateAttributes, // new attributes will be set in `attributes` field
}

struct LogMeta<ServiceMetaType, ServiceLogMetaType> {
    storage_item_common_meta!(ServiceMetaType)
    action: LogAction,
    service_meta: ServiceLogMetaType,
}

// meta for DMC, sample.
type ServiceMetaTypeDMC = ();
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
