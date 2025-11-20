#![allow(unused)]

use crate::translate_local_path_from_url;
use crate::BackupCheckpoint;
use crate::BackupChunkItem;
use crate::BackupItemState;
use crate::BackupResult;
use crate::BuckyBackupError;
use crate::CheckPointState;
use crate::ChunkInnerPathHelper;
use crate::RangeReader;
use crate::RemoteBackupCheckPointItemStatus;
use crate::CHECKPOINT_TYPE_CHUNK;
use async_trait::async_trait;
use log::*;
use ndn_lib::*;
use serde_json::json;
use serde_json::Value;
use std::cmp::min;
use std::collections::HashMap;
use std::collections::HashSet;
use std::future;
use std::future::Future;
use std::io::SeekFrom;
use std::mem;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::task::Waker;
use tokio::io::BufWriter;
use tokio::sync::Mutex;
use tokio::{
    fs::{self, File, OpenOptions},
    io::{self, AsyncRead, AsyncReadExt, AsyncSeek, AsyncSeekExt, AsyncWrite, AsyncWriteExt},
};
use url::{form_urlencoded::Target, Url};

use crate::provider::*;

//待备份的chunk都以文件的形式平摊的保存目录下
pub struct LocalDirChunkProvider {
    pub dir_path: PathBuf,
    pub named_mgr_id: String,
    pub is_strict_mode: bool,
}

impl LocalDirChunkProvider {
    pub async fn new(dir_path: String, named_mgr_id: String) -> BackupResult<Self> {
        info!("new local dir chunk provider, dir_path: {}", dir_path);
        Ok(LocalDirChunkProvider {
            dir_path: PathBuf::from(dir_path),
            named_mgr_id,
            is_strict_mode: false,
        })
    }
}

#[async_trait]
impl IBackupChunkSourceProvider for LocalDirChunkProvider {
    async fn get_source_info(&self) -> BackupResult<Value> {
        let result = json!({
            "name": "local_chunk_source",
            "desc": "local chunk source provider",
            "type_id": "local_chunk_source",
            "abilities": [ABILITY_LOCAL],
            "dir_path": self.dir_path,
        });
        Ok(result)
    }

    fn is_support(&self, ability: &str) -> bool {
        ability == ABILITY_LOCAL
    }

    fn is_local(&self) -> bool {
        true
    }

    fn get_source_url(&self) -> String {
        format!("file:///{}", self.dir_path.to_string_lossy())
    }

    async fn prepare_items(
        &self,
        checkpoint_id: &str,
        callback: Option<Arc<Mutex<NdnProgressCallback>>>,
    ) -> BackupResult<(Vec<BackupChunkItem>, u64, bool)> {
        let items = Arc::new(Mutex::new(Vec::<BackupChunkItem>::new()));
        let ndn_mgr_id = Some(self.named_mgr_id.as_str());
        let file_obj_template = FileObject::new("".to_string(), 0, "".to_string());
        let mut check_mode = CheckMode::ByQCID;
        if self.is_strict_mode {
            check_mode = CheckMode::ByFullHash;
        }

        let base_dir = self.dir_path.to_string_lossy().to_string();
        let base_dir_path = PathBuf::from(base_dir);

        let items_clone = items.clone();
        let base_dir_path_clone = base_dir_path.clone();
        let mut total_size = 0;
        let ndn_callback: Option<Arc<Mutex<NdnProgressCallback>>> = Some(Arc::new(Mutex::new(
            Box::new(move |inner_path: String, action: NdnAction| {
                let items_ref = items_clone.clone();
                let base_dir_path = base_dir_path_clone.clone();
                let callback_clone = callback.clone();
                Box::pin(async move {
                    debug!("ndn_callback: {} {}", inner_path, action.to_string());
                    let now = buckyos_kit::buckyos_get_unix_timestamp();
                    match action {
                        NdnAction::ChunkOK(chunk_id, chunk_size) => {
                            //将inner_path转换为相对路径,路径看起来是
                            // dirA/fileA/start:end -> chunk_id (大文件)
                            // dirA/fileB -> chunk_id (小文件)
                            let relative_path = Path::new(&inner_path).strip_prefix(&base_dir_path);
                            if relative_path.is_err() {
                                return Err(NdnError::InvalidState(format!(
                                    "relative path error: {}",
                                    inner_path
                                )));
                            }
                            let relative_path =
                                relative_path.unwrap().to_string_lossy().to_string();
                            let mut offset = 0;
                            if let Some(offset_pos) = inner_path.rfind('/') {
                                if let Some(offset_end) = inner_path.find(':') {
                                    offset =
                                        (&inner_path[offset_pos + 1..offset_end]).parse().expect(
                                            format!("inner-path format error: {}", inner_path)
                                                .as_str(),
                                        );
                                }
                            }
                            let backup_item = BackupChunkItem {
                                item_id: relative_path,
                                chunk_id: chunk_id,
                                local_chunk_id: None,
                                state: BackupItemState::New,
                                size: chunk_size,
                                last_update_time: now,
                                offset,
                            };

                            items_ref.lock().await.push(backup_item);
                            total_size += chunk_size;
                            Ok(ProgressCallbackResult::Continue)
                        }
                        NdnAction::FileOK(file_id, file_size) => {
                            if callback_clone.is_some() {
                                let callback_clone = callback_clone.unwrap();
                                let mut callback_clone = callback_clone.lock().await;
                                let ret = callback_clone(
                                    inner_path,
                                    NdnAction::FileOK(file_id, file_size),
                                )
                                .await?;
                                drop(callback_clone);
                                return Ok(ret);
                            }
                            Ok(ProgressCallbackResult::Continue)
                        }
                        _ => {
                            return Ok(ProgressCallbackResult::Continue);
                        }
                    }
                })
                    as Pin<
                        Box<
                            dyn std::future::Future<Output = NdnResult<ProgressCallbackResult>>
                                + Send
                                + 'static,
                        >,
                    >
            }),
        )));

        let ret = cacl_dir_object(
            ndn_mgr_id,
            &self.dir_path.as_path(),
            &file_obj_template,
            &check_mode,
            StoreMode::new_local(),
            ndn_callback,
        )
        .await;
        if ret.is_err() {
            return Err(BuckyBackupError::Failed(ret.err().unwrap().to_string()));
        }

        // 尝试直接取出 Vec，避免克隆
        let items_vec = match Arc::try_unwrap(items) {
            Ok(mutex) => mutex.into_inner(),
            Err(arc) => {
                // 如果还有多个引用（闭包可能仍持有），使用 mem::take 在锁内取出内容
                mem::take(&mut *arc.lock().await)
            }
        };
        Ok((items_vec, total_size, true))
    }

    async fn open_item_chunk_reader(
        &self,
        checkpoint_id: &str,
        backup_item: &BackupChunkItem,
        offset: u64,
    ) -> BackupResult<ChunkReader> {
        let reader = NamedDataMgr::open_chunk_reader(
            Some(&self.named_mgr_id.as_str()),
            &backup_item.chunk_id,
            offset,
            true,
        )
        .await
        .map_err(|e| {
            warn!("open_item_chunk_reader error:{}", e.to_string());
            BuckyBackupError::TryLater(e.to_string())
        })?;

        Ok(Box::pin(RangeReader::new(
            reader.0,
            backup_item.size - offset,
        )))
    }

    async fn open_chunk_reader(
        &self,
        chunk_id: &ChunkId,
        offset: u64,
    ) -> BackupResult<ChunkReader> {
        let reader = NamedDataMgr::open_chunk_reader(
            Some(&self.named_mgr_id.as_str()),
            chunk_id,
            offset,
            true,
        )
        .await
        .map_err(|e| {
            warn!("open_chunk_reader error:{}", e.to_string());
            BuckyBackupError::TryLater(e.to_string())
        })?;
        Ok(Box::pin(RangeReader::new(
            reader.0,
            chunk_id.get_length().unwrap() - offset,
        )))
    }

    //for resotre
    async fn add_checkpoint(&self, checkpoint: &BackupCheckpoint) -> BackupResult<()> {
        unimplemented!()
    }

    async fn init_for_restore(
        &self,
        restore_config: &RestoreConfig,
        checkpoint_id: &str,
    ) -> BackupResult<String> {
        unimplemented!()
    }

    async fn open_writer_for_restore(
        &self,
        restore_target_id: &str,
        item: &BackupChunkItem,
        restore_config: &RestoreConfig,
        offset: u64,
    ) -> BackupResult<(ChunkWriter, u64)> {
        let dir_path = PathBuf::from(translate_local_path_from_url(
            restore_config.restore_location_url.as_str(),
        )?);

        let item_relation_path = ChunkInnerPathHelper::strip_chunk_suffix(item.item_id.as_str());
        let target_file_full_path = dir_path.join(item_relation_path.as_str());

        // check exist chunk
        let file_len = match fs::try_exists(target_file_full_path.as_path()).await {
            Ok(is_exist) if !is_exist => 0,
            _ => match fs::metadata(target_file_full_path.as_path()).await {
                Ok(meta) => meta.len(),
                Err(err) => return Err(BuckyBackupError::Failed(err.to_string())),
            },
        };

        match file_len.cmp(&item.offset) {
            std::cmp::Ordering::Less => {
                // write to buffer and flush to file when the file-length equal the offset
                return Err(BuckyBackupError::TryLater("".to_string()));
            }
            std::cmp::Ordering::Equal => {
                // write to file
            }
            std::cmp::Ordering::Greater => {
                // check it
                if file_len >= item.offset + item.size {
                    let reader = fs::File::options()
                        .read(true)
                        .open(target_file_full_path.as_path())
                        .await;
                    if let Ok(mut reader) = reader {
                        let read_len_once = 16 * 1024 * 1024;
                        let mut read_pos = item.offset;
                        let mut buf = Vec::with_capacity(read_len_once as usize);
                        let mut chunk_hasher = ChunkHasher::new(Some(
                            item.chunk_id
                                .chunk_type
                                .to_hash_method()
                                .map_err(|err| {
                                    BuckyBackupError::Failed(format!(
                                        "invalid chunk-type: {:?}",
                                        item.chunk_id
                                    ))
                                })?
                                .as_str(),
                        ))
                        .map_err(|err| {
                            BuckyBackupError::Failed(format!(
                                "invalid chunk-type: {:?}",
                                item.chunk_id
                            ))
                        })?;
                        if reader.seek(SeekFrom::Start(read_pos)).await.is_ok() {
                            while read_pos < item.offset + item.size {
                                let read_len =
                                    min(read_len_once, item.offset + item.size - read_pos);
                                buf.resize(read_len as usize, 0);
                                if reader.read_exact(buf.as_mut_slice()).await.is_err() {
                                    break;
                                }
                                chunk_hasher.update_from_bytes(buf.as_slice());
                                read_pos = read_pos + read_len;
                            }

                            let is_read_ok = read_pos == item.offset + item.size;
                            if is_read_ok {
                                let exist_chunk_id = if item.chunk_id.chunk_type.is_mix() {
                                    chunk_hasher.finalize_mix_chunk_id().map_err(|err| {
                                        BuckyBackupError::Failed(format!(
                                            "faile mix-chunk: {:?}",
                                            err
                                        ))
                                    })?
                                } else {
                                    chunk_hasher.finalize_chunk_id()
                                };
                                if exist_chunk_id == item.chunk_id {
                                    return Err(BuckyBackupError::AlreadyDone("".to_string()));
                                }
                            }
                        }
                    }
                }
            }
        }

        if let Some(parent_dir) = target_file_full_path.parent() {
            fs::create_dir_all(parent_dir).await.map_err(|err| {
                BuckyBackupError::Failed(format!(
                    "Create parent directory failed: {:?}, {:?}",
                    parent_dir, err
                ))
            })?;
        }
        // write to file
        let mut writer = fs::File::options()
            .write(true)
            .create(true)
            .open(target_file_full_path.as_path())
            .await
            .map_err(|err| {
                BuckyBackupError::Failed(format!("Write failed: {}, {:?}", item.item_id, err))
            })?;
        writer
            .seek(SeekFrom::Start(item.offset))
            .await
            .map_err(|err| {
                BuckyBackupError::Failed(format!("Seek failed: {},  {:?}", item.item_id, err))
            })?;
        Ok((Box::pin(writer), 0))
    }
}

pub struct LocalChunkTargetProvider {
    pub dir_path: String,
    pub named_mgr_id: String,
}

impl LocalChunkTargetProvider {
    pub async fn new(dir_path: String, named_mgr_id: String) -> BackupResult<Self> {
        let root_path: PathBuf = PathBuf::from(dir_path.clone());
        let mgr_map = NAMED_DATA_MGR_MAP.lock().await;
        let the_named_mgr = mgr_map.get(named_mgr_id.as_str());
        if the_named_mgr.is_some() {
            let the_named_mgr = the_named_mgr.unwrap();
            let the_named_mgr = the_named_mgr.lock().await;
            if the_named_mgr.get_base_dir() != root_path {
                return Err(BuckyBackupError::Failed(format!(
                    "named_mgr {} base_dir not match: {}",
                    named_mgr_id,
                    the_named_mgr.get_base_dir().to_string_lossy()
                )));
            }
        } else {
            drop(mgr_map);
            let named_mgr = NamedDataMgr::get_named_data_mgr_by_path(root_path)
                .await
                .map_err(|e| {
                    BuckyBackupError::NeedProcess(format!(
                        "get named data mgr by path error: {}",
                        e.to_string()
                    ))
                })?;
            NamedDataMgr::set_mgr_by_id(Some(&named_mgr_id.as_str()), named_mgr)
                .await
                .map_err(|e| {
                    BuckyBackupError::NeedProcess(format!(
                        "set named data mgr by id error: {}",
                        e.to_string()
                    ))
                })?;
        }

        //create named data mgr ,if exists, return error.
        info!("new local chunk target provider, dir_path: {}", dir_path);
        Ok(LocalChunkTargetProvider {
            dir_path,
            named_mgr_id,
        })
    }
}

#[async_trait]
impl IBackupChunkTargetProvider for LocalChunkTargetProvider {
    async fn get_target_info(&self) -> BackupResult<String> {
        let result = json!({
            "type": "local_chunk_target",
            "dir_path": self.dir_path,
            "named_mgr_id": self.named_mgr_id,
        });
        Ok(result.to_string())
    }

    fn get_target_url(&self) -> String {
        format!("file:///{}", self.dir_path)
    }

    async fn get_account_session_info(&self) -> BackupResult<String> {
        Ok(String::new())
    }
    async fn set_account_session_info(&self, session_info: &str) -> BackupResult<()> {
        Ok(())
    }

    async fn alloc_checkpoint(&self, checkpoint: &BackupCheckpoint) -> BackupResult<()> {
        //check free space
        //if free space is not enough, return error
        return Ok(());
    }

    async fn add_backup_item(
        &self,
        checkpoint_id: &str,
        backup_items: &Vec<BackupChunkItem>,
    ) -> BackupResult<()> {
        return Ok(());
    }

    async fn query_check_point_state(
        &self,
        checkpoint_id: &str,
    ) -> BackupResult<(BackupCheckpoint, RemoteBackupCheckPointItemStatus)> {
        //return Ok((BackupCheckpoint::new(), RemoteBackupCheckPointItemStatus::NotSupport));
        let checkpoint = BackupCheckpoint {
            checkpoint_type: CHECKPOINT_TYPE_CHUNK.to_string(),
            checkpoint_name: checkpoint_id.to_string(),
            prev_checkpoint_id: None,
            state: CheckPointState::Working,
            extra_info: "".to_string(),
            create_time: 0,
            last_update_time: 0,
            item_list_id: "".to_string(),
            item_count: 0,
            total_size: 0,
        };
        Ok((checkpoint, RemoteBackupCheckPointItemStatus::NotSupport))
    }

    async fn remove_checkpoint(&self, checkpoint_id: &str) -> BackupResult<()> {
        unimplemented!()
    }

    async fn open_chunk_writer(
        &self,
        checkpoint_id: &str,
        chunk_id: &ChunkId,
        chunk_size: u64,
    ) -> BackupResult<(ChunkWriter, u64)> {
        let (writer, _progress) =
            NamedDataMgr::open_chunk_writer(Some(&self.named_mgr_id.as_str()), chunk_id, 0, 0)
                .await
                .map_err(|e| match e {
                    NdnError::AlreadyExists(msg) => BuckyBackupError::AlreadyDone(msg),
                    _ => BuckyBackupError::Failed(e.to_string()),
                })?;
        Ok((writer, 0))
    }

    async fn complete_chunk_writer(
        &self,
        checkpoint_id: &str,
        chunk_id: &ChunkId,
    ) -> BackupResult<()> {
        NamedDataMgr::complete_chunk_writer(Some(&self.named_mgr_id.as_str()), chunk_id)
            .await
            .map_err(|e| {
                warn!("complete_chunk_writer error:{}", e.to_string());
                BuckyBackupError::TryLater(e.to_string())
            })
    }

    async fn open_chunk_reader_for_restore(
        &self,
        chunk_id: &ChunkId,
        offset: u64,
    ) -> BackupResult<ChunkReader> {
        let reader = NamedDataMgr::open_chunk_reader(
            Some(&self.named_mgr_id.as_str()),
            chunk_id,
            offset,
            false,
        )
        .await
        .map_err(|e| {
            warn!("open_chunk_reader_for_restore error:{}", e.to_string());
            BuckyBackupError::TryLater(e.to_string())
        })?;

        Ok(Box::pin(RangeReader::new(
            reader.0,
            chunk_id.get_length().unwrap() - offset,
        )))
    }
}

// pub type WriteFileWithOffsetChecker = Box<dyn FnMut(&[u8]) -> bool + Send>;

// enum StepWriter {
//     Buffer(BufWriter<std::io::Cursor<Vec<u8>>>),
//     PreFile(Arc<std::sync::Mutex<Option<tokio::fs::File>>>),
//     File(tokio::fs::File),
//     Ready(future::Ready<Result<usize, std::io::Error>>),
// }

// impl StepWriter {
//     fn is_buffer(&self) -> bool {
//         matches!(self, StepWriter::Buffer(_))
//     }
//     fn is_file(&self) -> bool {
//         matches!(self, StepWriter::File(_))
//     }
//     fn is_ready(&self) -> bool {
//         matches!(self, StepWriter::Ready(_))
//     }
// }

// struct WriteState {
//     wakers: HashSet<Waker>,
//     err: Option<std::io::Error>,
// }

// impl WriteState {
//     fn new() -> Arc<std::sync::Mutex<Self>> {
//         Arc::new(std::sync::Mutex::new(WriteState {
//             wakers: HashSet::new(),
//             err: None,
//         }))
//     }
// }

// pub struct WriteFileWithOffset {
//     file_full_path: PathBuf,
//     offset: u64,
//     len: u64,
//     checker: WriteFileWithOffsetChecker,
//     writer: StepWriter,
//     state: Arc<std::sync::Mutex<WriteState>>,
// }

// impl WriteFileWithOffset {
//     pub async fn try_open(
//         file_full_path: PathBuf,
//         offset: u64,
//         len: u64,
//         mut checker: WriteFileWithOffsetChecker,
//     ) -> io::Result<Self> {
//         let file_len = match fs::try_exists(file_full_path.as_path()).await {
//             Ok(is_exist) if !is_exist => 0,
//             _ => fs::metadata(file_full_path.as_path()).await?.len(),
//         };

//         match file_len.cmp(&offset) {
//             std::cmp::Ordering::Less => {
//                 let buffer = vec![];
//                 // write to buffer and flush to file when the file-length equal the offset
//                 return Ok(Self {
//                     file_full_path,
//                     offset,
//                     len,
//                     checker,
//                     writer: StepWriter::Buffer(BufWriter::new(std::io::Cursor::new(buffer))),
//                     state: WriteState::new(),
//                 });
//             }
//             std::cmp::Ordering::Equal => {
//                 // write to file
//             }
//             std::cmp::Ordering::Greater => {
//                 if file_len >= offset + len {
//                     // check it
//                     let mut reader = fs::File::options()
//                         .read(true)
//                         .open(file_full_path.as_path())
//                         .await?;
//                     let read_len_once = 16 * 1024 * 1024;
//                     let mut read_pos = 0;
//                     while read_pos < offset + len {
//                         let read_len = min(read_len_once, offset + len - read_pos);
//                         let mut buf = Vec::with_capacity(read_len as usize);
//                         buf.resize(read_len as usize, 0);
//                         reader.read_exact(buf.as_mut_slice()).await;
//                         if !checker.as_mut()(buf.as_slice()) {
//                             drop(reader);
//                             // write to file
//                             let mut writer = fs::File::options()
//                                 .write(true)
//                                 .open(file_full_path.as_path())
//                                 .await?;
//                             writer.seek(SeekFrom::Start(offset)).await?;
//                             return Ok(Self {
//                                 file_full_path,
//                                 offset,
//                                 len,
//                                 checker,
//                                 writer: StepWriter::File(writer),
//                                 state: WriteState::new(),
//                             });
//                         }
//                     }
//                     return Ok(Self {
//                         file_full_path,
//                         offset,
//                         len,
//                         checker,
//                         writer: StepWriter::Ready(future::ready(Ok(0))),
//                         state: WriteState::new(),
//                     });
//                 } else {
//                     // write to file
//                 }
//             }
//         }

//         let mut writer = fs::File::options()
//             .write(true)
//             .append(true)
//             .create_new(true)
//             .open(file_full_path.as_path())
//             .await?;
//         writer.seek(SeekFrom::Start(offset)).await?;
//         return Ok(Self {
//             file_full_path,
//             offset,
//             len,
//             checker,
//             writer: StepWriter::File(writer),
//             state: WriteState::new(),
//         });
//     }

//     fn prepare_file_writer(&self, buffer: Vec<u8>) {
//         let file_path = self.file_full_path.clone();
//         let offset = self.offset;
//         let writer_state = self.state.clone();
//         let prepare_file = if let StepWriter::PreFile(file) = &self.writer {
//             file.clone()
//         } else {
//             unreachable!("only pre-file")
//         };

//         let check_and_open_writer =
//             async |path: &Path, offset: u64| -> std::io::Result<Option<File>> {
//                 let meta = fs::metadata(path).await?;
//                 if meta.len() >= offset {
//                     let mut writer = fs::File::options()
//                         .write(true)
//                         .append(true)
//                         .create_new(true)
//                         .open(path)
//                         .await?;
//                     writer.seek(SeekFrom::Start(offset)).await?;
//                     Ok(Some(writer))
//                 } else {
//                     Ok(None)
//                 }
//             };

//         tokio::spawn(async move {
//             let mut err = None;
//             loop {
//                 match check_and_open_writer().await {
//                     Ok(writer) => {
//                         if let Some(writer) = writer {
//                             prepare_file.lock().unwrap().replace(writer);
//                             break;
//                         }
//                     }
//                     Err(err) => {
//                         err = Some(err);
//                         break;
//                     }
//                 }

//                 let state = writer_state.lock().unwrap();
//                 state.waker
//             }
//         });
//     }

//     fn set_err(&mut self, err: std::io::Error) {
//         self.state.lock().unwrap().err.get_or_insert(err);
//     }

//     fn check_err(&self) -> Option<std::io::Error> {
//         let mut state = self.state.lock().unwrap();
//         let err_code = state.err.as_ref().map(|e| (e.raw_os_error(), e.kind()));
//         match err_code {
//             Some((code, kind)) => {
//                 let new_err = match code {
//                     Some(code) => std::io::Error::from_raw_os_error(code),
//                     None => std::io::Error::new(kind, state.err.as_ref().unwrap().to_string()),
//                 };
//                 state.err.replace(new_err)
//             }
//             None => None,
//         }
//     }
// }

// impl AsyncWrite for WriteFileWithOffset {
//     fn poll_write(
//         self: Pin<&mut Self>,
//         cx: &mut std::task::Context<'_>,
//         buf: &[u8],
//     ) -> std::task::Poll<Result<usize, std::io::Error>> {
//         // ready
//         let writer = self.get_mut();
//         if let StepWriter::Ready(ready) = &mut writer.writer {
//             let mut pinned = std::pin::pin!(ready);
//             return pinned.poll(cx);
//         }

//         // failed
//         let err = writer.check_err();
//         if let Some(err) = err {
//             return std::task::Poll::Ready(Err(err));
//         }

//         let (buffer, file) = match &mut writer.writer {
//             StepWriter::Buffer(buf_writer) => {
//                 let mut pinned = std::pin::pin!(buf_writer);
//                 let state = pinned.as_mut().poll_write(cx, buf);
//                 match state {
//                     std::task::Poll::Pending => {
//                         writer
//                             .state
//                             .lock()
//                             .unwrap()
//                             .wakers
//                             .insert(cx.waker().clone());
//                         return std::task::Poll::Pending;
//                     }
//                     std::task::Poll::Ready(result) => match result {
//                         Ok(len) => (Some(buf_writer.into_inner().into_inner()), None),
//                         Err(err) => {
//                             writer.set_err(err);
//                             return std::task::Poll::Ready(Err(writer.check_err().unwrap()));
//                         }
//                     },
//                 }
//             }
//             StepWriter::PreFile(file) => {
//                 let mut file = file.lock().unwrap();
//                 (None, file.take())
//             }
//             StepWriter::File(file) => {
//                 let mut pinned = std::pin::pin!(file);
//                 match pinned.poll_write(cx, buf) {
//                     std::task::Poll::Ready(ret) => match ret {
//                         Ok(len) => return std::task::Poll::Ready(Ok(len)),
//                         Err(err) => {
//                             writer.set_err(err);
//                             return std::task::Poll::Ready(Err(writer.check_err().unwrap()));
//                         }
//                     },
//                     std::task::Poll::Pending => return std::task::Poll::Pending,
//                 }
//             }
//             StepWriter::Ready(ready) => unreachable!("proccessed before"),
//         };

//         if let Some(buffer) = buffer {
//             writer.writer = StepWriter::PreFile(Arc::new(std::sync::Mutex::new(None)));
//             writer.prepare_file_writer(buffer);
//         }
//         if let Some(file) = file {
//             writer.writer = StepWriter::File(file);
//         }
//         std::task::Poll::Pending
//     }

//     fn poll_flush(
//         self: Pin<&mut Self>,
//         cx: &mut std::task::Context<'_>,
//     ) -> std::task::Poll<Result<(), std::io::Error>> {
//         todo!()
//     }

//     fn poll_shutdown(
//         self: Pin<&mut Self>,
//         cx: &mut std::task::Context<'_>,
//     ) -> std::task::Poll<Result<(), std::io::Error>> {
//         todo!()
//     }
// }

// impl Drop for WriteFileWithOffset {
//     fn drop(&mut self) {
//         todo!("wake other writers and free wakers, set err to other states")
//     }
// }
