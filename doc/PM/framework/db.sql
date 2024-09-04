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
    source_param TEXT DEFAULT NULL,
    target_id INTEGER NOT NULL,
    target_param TEXT DEFAULT NULL,
    attachment TEXT DEFAULT NULL,
    priority INTEGER NOT NULL,
    history_strategy TEXT DEFAULT NULL,
    flag: INTEGER DEFAULT 0,
    FOREIGN KEY (source_id) REFERENCES sources (id),
    FOREIGN KEY (target_id) REFERENCES targets (id)
);

-- create table `task_source_state` with the following columns:
-- id: u64
-- task_uuid: String
-- is_preserved: bool // false if the source has not preserved its state
-- original_state: Option<String>,
-- preserved_state: Option<String>,
CREATE TABLE IF NOT EXISTS task_source_state (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    task_uuid TEXT NOT NULL,
    is_preserved INTEGER DEFAULT 0,
    original_state TEXT DEFAULT NULL,
    preserved_state TEXT DEFAULT NULL,
    FOREIGN KEY (task_uuid) REFERENCES tasks (uuid)
);

-- create table `checkpoints` with the following columns:
-- task_uuid: String,
-- seq: u64, // +1 for each new checkpoint for the same task
-- create_time: u64, // UTC in seconds
-- last_status_changed_time: u64, // UTC in seconds
-- preserved_source_state_id: Option<u64>,
-- meta: String,
-- target_meta: Option<Vec<String>>,
-- status: u64,
-- error_msg: Option<String>,
-- (task_uuid, seq, create_time) is primary key
CREATE TABLE IF NOT EXISTS checkpoints (
    task_uuid TEXT NOT NULL,
    seq INTEGER NOT NULL,
    create_time INTEGER NOT NULL,
    last_status_changed_time INTEGER NOT NULL,
    preserved_source_state_id INTEGER DEFAULT NULL,
    meta TEXT NOT NULL,
    target_meta TEXT DEFAULT NULL,
    status INTEGER NOT NULL,
    error_msg TEXT DEFAULT NULL,
    PRIMARY KEY (task_uuid, seq, create_time),
    FOREIGN KEY (task_uuid) REFERENCES tasks (uuid),
    FOREIGN KEY (preserved_source_state_id) REFERENCES task_source_state (id)
);

-- create table `checkpoint_transfer_map` with the following columns:
-- task_uuid: String,
-- seq: u64,
-- checkpoint_create_time: u64, // (task_uuid, seq, checkpoint_create_time) REFERENCES checkpoints
-- item_path: Vec<u8>, // (task_uuid, seq, checkpoint_create_time, item_path) is primary key
-- offset: u64,
-- length: u64,
-- begin_time: u64,
-- finish_time: u64,
-- target_address: Vec<u8>,
-- detail: Vec<u8>,
CREATE TABLE IF NOT EXISTS `checkpoint_transfer_map` (
    task_uuid TEXT NOT NULL,
    seq INTEGER NOT NULL,
    checkpoint_create_time INTEGER NOT NULL,
    item_path BLOB NOT NULL,
    offset INTEGER NOT NULL,
    length INTEGER NOT NULL,
    begin_time INTEGER NOT NULL,
    finish_time INTEGER DEFAULT NULL,
    target_address BLOB DEFAULT NULL,
    detail BLOB DEFAULT NULL,
    PRIMARY KEY (task_uuid, seq, checkpoint_create_time, item_path),
    FOREIGN KEY (task_uuid, seq, checkpoint_create_time) REFERENCES checkpoints (task_uuid, seq, create_time)
);

-- create table `checkpoint_key_value` with the following columns:
-- task_uuid: String,
-- seq: u64,
-- checkpoint_create_time: u64, // (task_uuid, seq, checkpoint_create_time) REFERENCES checkpoints
-- key: Option<String>, // (task_uuid, seq, checkpoint_create_time, key) is primary key
-- value: String,
CREATE TABLE IF NOT EXISTS `checkpoint_key_value` (
    task_uuid TEXT NOT NULL,
    seq INTEGER NOT NULL,
    checkpoint_create_time INTEGER NOT NULL,
    key TEXT DEFAULT NULL,
    value TEXT NOT NULL,
    PRIMARY KEY (task_uuid, seq, checkpoint_create_time, key),
    FOREIGN KEY (task_uuid, seq, checkpoint_create_time) REFERENCES checkpoints (task_uuid, seq, create_time)
);