-- create table `sources` with the following columns:
-- pub struct SourceInfo {
--     pub id: SourceId,
--     pub classify: String,
--     pub url: String, // url is unique key
--     pub friendly_name: String
--     pub config: String
--     pub description: String,
-- }
CREATE TABLE IF NOT EXISTS sources (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    classify TEXT NOT NULL,
    url TEXT NOT NULL,
    friendly_name TEXT NOT NULL,
    config TEXT NOT NULL,
    description TEXT DEFAULT NULL,
    UNIQUE(url)
);

-- create table `targets` with the following columns:
-- pub struct TargetInfo {
--    pub id: SourceId,
--    pub classify: String,
--    pub url: String,
--    pub friendly_name: String
--    pub config: String
--    pub description: String,
-- }
CREATE TABLE IF NOT EXISTS targets (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    classify TEXT NOT NULL,
    url TEXT NOT NULL,
    friendly_name TEXT NOT NULL,
    config TEXT NOT NULL,
    description TEXT DEFAULT NULL,
    UNIQUE(url)
);

-- create table `config` with the following columns:
-- category: String, // constraint: unique
-- config: String
CREATE TABLE IF NOT EXISTS config (
    category TEXT PRIMARY KEY,
    config TEXT NOT NULL
);

-- create table `tasks` with the following columns:
-- pub struct TaskInfo {
--     pub uuid: String,
--     pub friendly_name: String,
--     pub description: String,
--     pub source_id: SourceId,
--     pub source_param: String, // Any parameters(address .eg) for the source, the source can get it from engine.
--     pub target_id: String,
--     pub target_param: String, // Any parameters(address .eg) for the target, the target can get it from engine.
--     pub priority: u32,
--     pub attachment: String,   // The application can save any attachment with task.
--     pub history_strategy: String,
--     pub flag: u64, // Save any flags for the task. it will be filterd when list the tasks.
-- }
CREATE TABLE IF NOT EXISTS tasks (
    uuid TEXT PRIMARY KEY,
    friendly_name TEXT NOT NULL,
    description TEXT DEFAULT NULL,
    source_id INTEGER NOT NULL,
    source_entitiy TEXT DEFAULT NULL,
    target_id INTEGER NOT NULL,
    target_entitiy TEXT DEFAULT NULL,
    attachment TEXT DEFAULT NULL,
    priority INTEGER NOT NULL,
    history_strategy TEXT DEFAULT NULL,
    flag INTEGER DEFAULT 0,
    is_delete_from_target INTEGER DEFAULT NULL,
    FOREIGN KEY (source_id) REFERENCES sources (id),
    FOREIGN KEY (target_id) REFERENCES targets (id)
);

-- create table `task_source_state` with the following columns:
-- id: u64
-- task_uuid: String
-- is_locked: bool // false if the source has not locked its state
-- original_state: Option<String>,
-- locked_state: Option<String>,
CREATE TABLE IF NOT EXISTS locked_source_state (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    task_uuid TEXT NOT NULL,
    lock INTEGER DEFAULT 0, -- 0: None, 1: locked, 2: unlocked
    original_state TEXT DEFAULT NULL,
    locked_state TEXT DEFAULT NULL,
    check_point_seq INTEGER DEFAULT NULL,
    check_point_create_time INTEGER DEFAULT NULL,
    create_time INTEGER NOT NULL,
    creator_magic INTEGER NOT NULL,
    FOREIGN KEY (task_uuid) REFERENCES tasks (uuid),
    UNIQUE(task_uuid, check_point_seq, check_point_create_time)
);

-- create table `checkpoints` with the following columns:
-- task_uuid: String,
-- seq: u64, // +1 for each new checkpoint for the same task
-- create_time: u64, // UTC in seconds
-- last_status_changed_time: u64, // UTC in seconds
-- locked_source_state_id: u64,
-- status: u64,
-- error_msg: Option<String>,
-- (task_uuid, seq, create_time) is primary key
-- (task_uuid, seq) is unique key
CREATE TABLE IF NOT EXISTS checkpoints (
    task_uuid TEXT NOT NULL,
    seq INTEGER NOT NULL,
    create_time INTEGER NOT NULL,
    prev_version_seq INTEGER DEFAULT NULL,
    prev_version_create_time INTEGER DEFAULT NULL,
    last_status_changed_time INTEGER NOT NULL,
    locked_source_state_id INTEGER NOT NULL,
    status INTEGER NOT NULL,
    error_msg TEXT DEFAULT NULL,
    complete_time INTEGER DEFAULT NULL,
    is_compress INTEGER DEFAULT 0,
    PRIMARY KEY (task_uuid, seq, create_time),
    FOREIGN KEY (task_uuid) REFERENCES tasks (uuid),
    FOREIGN KEY (locked_source_state_id) REFERENCES locked_source_state (id),
    UNIQUE(task_uuid, seq)
);

CREATE TABLE IF NOT EXISTS folder_files (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    task_uuid TEXT NOT NULL,
    checkpoint_seq INTEGER NOT NULL,
    checkpoint_create_time INTEGER NOT NULL,
    parent_id INTEGER DEFAULT NULL,
    file_path TEXT NOT NULL,
    file_type INTEGER NOT NULL,
    file_size INTEGER DEFAULT 0,
    attributes TEXT NOT NULL,
    update_action INTEGER DEFAULT NULL,
    FOREIGN KEY (task_uuid, checkpoint_seq, checkpoint_create_time) REFERENCES checkpoints(task_uuid, seq, create_time),
    FOREIGN KEY (parent_id) REFERENCES folder_files(id),
    UNIQUE (task_uuid, checkpoint_seq, checkpoint_create_time, file_path)
)

CREATE TABLE IF NOT EXISTS folder_file_custom_diffs (
    file_id INTEGER PRIMARY KEY,
    diff_classify TEXT NOT NULL,
    diff_header TEXT NOT NULL,
    FOREIGN KEY (file_id) REFERENCES folder_files(id)
)

CREATE TABLE IF NOT EXISTS folder_file_default_diff_blocks (
    file_id INTEGER PRIMARY KEY,
    original_offset INTEGER NOT NULL,
    original_len INTEGER DEFAULT NULL,
    new_file_offset INTEGER DEFAULT NULL,
    diff_file_offset INTEGER DEFAULT NULL,
    new_len INTEGER DEFAULT NULL,
    FOREIGN KEY (file_id) REFERENCES folder_files(id),
    UNIQUE(file_id, original_offset),
    UNIQUE(file_id, new_file_offset),
    UNIQUE(file_id, diff_file_offset)
)

CREATE TABLE IF NOT EXISTS folder_file_links (
    file_id INTEGER PRIMARY KEY,
    target TEXT NOT NULL,
    is_hard INTEGER DEFAULT 0,
    FOREIGN KEY (file_id) REFERENCES folder_files(id)
)

-- `hash` and `size` will be set when the chunk is filled complete. 
CREATE TABLE IF NOT EXISTS chunks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    task_uuid TEXT NOT NULL,
    checkpoint_seq INTEGER NOT NULL,
    checkpoint_create_time INTEGER NOT NULL,
    chunk_hash TEXT DEFAULT NULL,
    size INTEGER DEFAULT 0,
    compress TEXT DEFAULT NULL,
    FOREIGN KEY (task_uuid, checkpoint_seq, checkpoint_create_time) REFERENCES checkpoints(task_uuid, checkpoint_seq, checkpoint_create_time),
    UNIQUE(chunk_hash)
)

CREATE TABLE IF NOT EXISTS chunk_items (
    chunk_item_id INTEGER PRIMARY KEY AUTOINCREMENT,
    chunk_id INTEGER NOT NULL,
    file_path_or_chunk_hash TEXT NOT NULL, -- file-path for files, chunk-hash for chunk
    parent_id INTEGER DEFAULT NULL,
    file_type INTEGER NOT NULL,
    attributes TEXT NOT NULL,
    update_action INTEGER DEFAULT NULL,
    FOREIGN KEY (chunk_id) REFERENCES chunks(id),
    FOREIGN KEY (parent_id) REFERENCES chunk_files(chunk_file_id),
    UNIQUE(chunk_id, file_path)
)

CREATE TABLE IF NOT EXISTS chunk_blocks (
    chunk_item_id INTEGER PRIMARY KEY,
    file_size INTEGER DEFAULT 0,
    diff_size INTEGER DEFAULT NULL,
    block_size INTEGER DEFAULT 0,
    chunk_pos INTEGER NOT NULL,
    file_offset INTEGER NOT NULL,
    compressed_size INTEGER DEFAULT NULL,
    FOREIGN KEY (chunk_file_id) REFERENCES chunk_files(chunk_file_id),
)

CREATE TABLE IF NOT EXISTS chunk_links (
    chunk_item_id INTEGER PRIMARY KEY,
    target TEXT NOT NULL,
    is_hard INTEGER DEFAULT 0,
    FOREIGN KEY (chunk_file_id) REFERENCES chunk_files(chunk_file_id),
)

-- -- create table `checkpoint_transfer_map` with the following columns:
-- -- task_uuid: String,
-- -- seq: u64,
-- -- checkpoint_create_time: u64, // (task_uuid, seq, checkpoint_create_time) REFERENCES checkpoints
-- -- item_path: Vec<u8>, // (task_uuid, seq, checkpoint_create_time, item_path) is primary key
-- -- offset: u64,
-- -- length: u64,
-- -- begin_time: u64,
-- -- finish_time: u64,
-- -- target_address: Vec<u8>,
-- -- detail: Vec<u8>,
-- CREATE TABLE IF NOT EXISTS `checkpoint_transfer_map` (
--     task_uuid TEXT NOT NULL,
--     seq INTEGER NOT NULL,
--     checkpoint_create_time INTEGER NOT NULL,
--     item_path BLOB NOT NULL,
--     offset INTEGER NOT NULL,
--     length INTEGER NOT NULL,
--     begin_time INTEGER NOT NULL,
--     finish_time INTEGER DEFAULT NULL,
--     target_address BLOB DEFAULT NULL,
--     detail BLOB DEFAULT NULL,
--     PRIMARY KEY (task_uuid, seq, checkpoint_create_time, item_path),
--     FOREIGN KEY (task_uuid, seq, checkpoint_create_time) REFERENCES checkpoints (task_uuid, seq, create_time)
-- );

-- -- create table `checkpoint_key_value` with the following columns:
-- -- task_uuid: String,
-- -- seq: u64,
-- -- checkpoint_create_time: u64, // (task_uuid, seq, checkpoint_create_time) REFERENCES checkpoints
-- -- key: Option<String>, // (task_uuid, seq, checkpoint_create_time, key) is primary key
-- -- value: String,
-- CREATE TABLE IF NOT EXISTS `checkpoint_key_value` (
--     task_uuid TEXT NOT NULL,
--     seq INTEGER NOT NULL,
--     checkpoint_create_time INTEGER NOT NULL,
--     key TEXT DEFAULT NULL,
--     value TEXT NOT NULL,
--     PRIMARY KEY (task_uuid, seq, checkpoint_create_time, key),
--     FOREIGN KEY (task_uuid, seq, checkpoint_create_time) REFERENCES checkpoints (task_uuid, seq, create_time)
-- );