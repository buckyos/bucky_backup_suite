#![allow(dead_code)]
use async_trait::async_trait;
use buckyos_backup_lib::{IBackupChunkTargetProvider, BackupResult, BuckyBackupError};
use ndn_lib::{ChunkId, ChunkReader, ChunkWriter};
use anyhow::{Result, anyhow};
use aws_sdk_s3::{Client, Config};
use aws_config::meta::region::RegionProviderChain;
use aws_credential_types::provider::{ProvideCredentials, SharedCredentialsProvider};
use aws_credential_types::Credentials;
use aws_config::BehaviorVersion;
use std::future::Future;
use std::task::{Context, Poll};
use std::{collections::HashMap, pin::Pin};
use std::sync::Mutex;
use aws_sdk_s3::types::{CompletedMultipartUpload, CompletedPart, MetadataDirective};
use serde::{Serialize, Deserialize};
use tokio::io::AsyncWrite;
use futures::FutureExt;  
use url::Url;

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum S3AccountSession {
    #[serde(rename = "env")]
    Environment,
    #[serde(rename = "key")]
    AccessKey {
        access_key_id: String,
        secret_access_key: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        session_token: Option<String>,
    }
}

enum UploadCreateState {
    Creating,
    Created(String), // upload_id
}

struct MultipartUploadState {
    create_state: UploadCreateState,
    total_size: u64,
}

impl MultipartUploadState {
    fn new(total_size: u64) -> Self {
        Self {
            create_state: UploadCreateState::Creating,
            total_size,
        }
    }

    fn set_created(&mut self, upload_id: String) {
        self.create_state = UploadCreateState::Created(upload_id);
    }

    fn get_upload_id(&self) -> Option<&str> {
        match &self.create_state {
            UploadCreateState::Created(id) => Some(id),
            _ => None,
        }
    }
}

pub struct S3ChunkTarget {
    client: Client,
    bucket: String,
    upload_states: Mutex<HashMap<String, MultipartUploadState>>, 
    url: String,
}

impl S3ChunkTarget {
    pub async fn with_url(url:Url) -> Result<Self> {
        // s3://bucket-name?region=region-name&access_key=xxx&secret_key=yyy
        let bucket = url.host_str().unwrap_or_default().to_string();
        let region = url.query_pairs().find(|(k, _)| k == "region").map(|(_, v)| v.to_string());
        let access_key = url.query_pairs().find(|(k, _)| k == "access_key").map(|(_, v)| v.to_string());
        let secret_key = url.query_pairs().find(|(k, _)| k == "secret_key").map(|(_, v)| v.to_string());
        let session_token = url.query_pairs().find(|(k, _)| k == "session_token").map(|(_, v)| v.to_string());
        let account = if access_key.is_none() || secret_key.is_none() {
            S3AccountSession::Environment
        } else {
            S3AccountSession::AccessKey {
                access_key_id: access_key.unwrap(),
                secret_access_key: secret_key.unwrap(),
                session_token,
            }
        };
        Self::with_session(bucket, region, account).await
    }

    pub async fn with_session(
        bucket: String, 
        region: Option<String>,
        session: S3AccountSession,
    ) -> Result<Self> {
        let region_provider = RegionProviderChain::first_try(region.clone().map(aws_config::Region::new))
            .or_default_provider();

        let config_builder = aws_config::defaults(BehaviorVersion::latest())
            .region(region_provider);

        let config = match &session {
            S3AccountSession::Environment => config_builder.load().await,
            S3AccountSession::AccessKey { 
                access_key_id, 
                secret_access_key, 
                session_token 
            } => {
                let credentials_provider = ProvideCredentials::provide_credentials(
                    &SharedCredentialsProvider::new(
                        Credentials::new(
                            access_key_id,
                            secret_access_key,
                            session_token.clone(),
                            None,
                            "s3-chunk-target",
                        )
                    )
                ).await.map_err(|e| anyhow!("Failed to create credentials: {}", e))?;

                config_builder
                    .credentials_provider(credentials_provider)
                    .load()
                    .await
            }
        };

        let s3_config = Config::new(&config);
        let client = Client::from_conf(s3_config);
        
        // 用bucket, region 和 account 生成url
        let mut params = vec![];

        if let Some(region) = region {
            params.push(("region", region));
        }

        if let S3AccountSession::AccessKey { access_key_id, secret_access_key, session_token } = session {
            params.push(("access_key", access_key_id));
            params.push(("secret_key", secret_access_key));
            if let Some(session_token) = session_token {
                params.push(("session_token", session_token));
            }
        }

        Ok(Self {
            client,
            upload_states: Mutex::new(HashMap::new()), 
            url: Url::parse_with_params(&format!("s3://{}", bucket), params).unwrap().to_string(),
            bucket,
        })
    }
}


struct UploadingState {
    upload_part_future: Pin<Box<dyn Future<Output = Result<()>> + Send>>,
    upload_size: usize,
}

enum UploadState {
    None, 
    Uploading(UploadingState),
    Err(String),
}

struct WriterState {
    uploaded_size: u64,
    part_limit: usize, 
    part_buffer: Vec<u8>,
    upload_state: UploadState,
}

struct S3ChunkWriter {
    client: Client,
    bucket: String,
    key: String,
    upload_id: String,
    chunk_size: u64,
    state: Mutex<WriterState>,
}

impl S3ChunkWriter {
    fn part_size() -> usize {
        5 * 1024 * 1024
    }

    async fn upload_part(client: Client, bucket: String, key: String, upload_id: String, data: Vec<u8>, part_number: i32) -> Result<()> { 
        let _ = client
            .upload_part()
            .bucket(&bucket)
            .key(&key)
            .upload_id(&upload_id)
            .part_number(part_number)
            .body(data.into())
            .send()
            .await
            .map_err(|e| anyhow!("Failed to upload part: {}", e))?;
        Ok(())
    }


    fn poll_write_part(
        &self,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<(bool, usize)>> {
        let mut state = self.state.lock().unwrap();
        let write_size = state.part_limit - state.part_buffer.len();
        
        if write_size > buf.len() {
            // 如果写入的数据小于part_limit，则直接写入part_buffer
            state.part_buffer.extend_from_slice(buf);
            Poll::Ready(Ok((false, buf.len())))
        } else if write_size > 0 {
            // 如果写入的数据大于0，则将数据写入part_buffer，并创建新的ToUploadPart
            state.part_buffer.extend_from_slice(&buf[..write_size]);
            let to_continue = if let UploadState::None = &state.upload_state {
                let mut part_buffer = Vec::new();
                std::mem::swap(&mut state.part_buffer, &mut part_buffer);
                state.part_limit = usize::min(S3ChunkWriter::part_size(), (self.chunk_size - (state.uploaded_size + part_buffer.len() as u64)) as usize);
                let part_number = (state.uploaded_size / Self::part_size() as u64 + 1) as i32;
                let upload_size = part_buffer.len();
                let mut upload_part_future = Box::pin(Self::upload_part(self.client.clone(), self.bucket.clone(), self.key.clone(), self.upload_id.clone(), part_buffer, part_number));
                match upload_part_future.poll_unpin(cx) {
                    Poll::Ready(result) => {
                        match result {
                            Ok(_) => {
                                state.upload_state = UploadState::None;
                                state.uploaded_size += upload_size as u64;
                                true
                            },
                            Err(e) => {
                                state.upload_state = UploadState::Err(e.to_string());
                                return Poll::Ready(Err(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())));
                            }
                        }
                    }, 
                    Poll::Pending => {
                        state.upload_state = UploadState::Uploading(UploadingState {
                            upload_part_future,
                            upload_size,
                        });
                        false
                    }
                }
            } else {
                false
            };
            Poll::Ready(Ok((to_continue, write_size)))
        } else {
            // 如果写入的数据为0，等待upload
            let to_continue = if let UploadState::Uploading(uploading_state) = &mut state.upload_state {
                match uploading_state.upload_part_future.as_mut().poll(cx) {
                    Poll::Ready(Ok(_)) => {
                        state.uploaded_size += uploading_state.upload_size as u64;
                        if state.part_buffer.len() == state.part_limit {
                            let mut part_buffer = Vec::new();
                            std::mem::swap(&mut state.part_buffer, &mut part_buffer);
                            state.part_limit = usize::min(S3ChunkWriter::part_size(), (self.chunk_size - (state.uploaded_size + part_buffer.len() as u64)) as usize);
                            let part_number = (state.uploaded_size / Self::part_size() as u64 + 1) as i32;
                            let upload_size = part_buffer.len();
                            let mut upload_part_future = Box::pin(Self::upload_part(self.client.clone(), self.bucket.clone(), self.key.clone(), self.upload_id.clone(), part_buffer, part_number));
                            match upload_part_future.poll_unpin(cx) {
                                Poll::Ready(result) => {
                                    match result {
                                        Ok(_) => {
                                            state.upload_state = UploadState::None;
                                            state.uploaded_size += upload_size as u64;
                                        },
                                        Err(e) => {
                                            state.upload_state = UploadState::Err(e.to_string());
                                            return Poll::Ready(Err(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())));
                                        }
                                    }
                                }, 
                                Poll::Pending => {
                                    state.upload_state = UploadState::Uploading(UploadingState {
                                        upload_part_future,
                                        upload_size,
                                    });
                                }
                            }
                        } else {
                            state.upload_state = UploadState::None;
                        }
                        true
                    },
                    Poll::Ready(Err(e)) => {
                        state.upload_state = UploadState::Err(e.to_string());
                        return Poll::Ready(Err(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())));
                    },
                    Poll::Pending => {
                        false
                    }
                }
            } else {
                unreachable!()
            };
            Poll::Ready(Ok((to_continue, 0)))
        }
    }
}


impl AsyncWrite for S3ChunkWriter {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        let mut_self = self.get_mut();
        {
            let state = mut_self.state.lock().unwrap();
            if let UploadState::Err(e) = &state.upload_state {
                return Poll::Ready(Err(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())));
            }
        }

        let mut total_write_size = 0;
        loop {
            match mut_self.poll_write_part(cx, &buf[total_write_size..]) {
                Poll::Ready(Ok((to_continue, write_size))) => {
                    total_write_size += write_size;
                    if !to_continue {
                        return Poll::Ready(Ok(total_write_size));
                    }
                }, 
                Poll::Ready(Err(e)) => {
                    return Poll::Ready(Err(e));
                },
                Poll::Pending => {
                    return Poll::Pending;
                }
            }
        }

    }

    fn poll_flush(
        self: Pin<&mut Self>, 
        cx: &mut Context<'_>
    ) -> Poll<Result<(), std::io::Error>> {
        // 如果缓冲区有数据，上传它
        let mut_self = self.get_mut();
        let mut state = mut_self.state.lock().unwrap();
        if let UploadState::Uploading(uploading_state) = &mut state.upload_state {
            match uploading_state.upload_part_future.as_mut().poll(cx) {
                Poll::Ready(Ok(_)) => {
                    state.uploaded_size += uploading_state.upload_size as u64;
                    if state.part_buffer.len() == state.part_limit {
                        let mut part_buffer = Vec::new();
                        std::mem::swap(&mut state.part_buffer, &mut part_buffer);
                        state.part_limit = usize::min(S3ChunkWriter::part_size(), (mut_self.chunk_size - (state.uploaded_size + part_buffer.len() as u64)) as usize);
                        let part_number = (state.uploaded_size / Self::part_size() as u64 + 1) as i32;
                        let upload_size = part_buffer.len();
                        let mut upload_part_future = Box::pin(Self::upload_part(mut_self.client.clone(), mut_self.bucket.clone(), mut_self.key.clone(), mut_self.upload_id.clone(), part_buffer, part_number));
                        match upload_part_future.poll_unpin(cx) {
                            Poll::Ready(result) => {
                                match result {
                                    Ok(_) => {
                                        state.upload_state = UploadState::None;
                                        state.uploaded_size += upload_size as u64;
                                        Poll::Ready(Ok(()))
                                    },
                                    Err(e) => {
                                        state.upload_state = UploadState::Err(e.to_string());
                                        Poll::Ready(Err(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))
                                    }
                                }
                            }, 
                            Poll::Pending => {
                                state.upload_state = UploadState::Uploading(UploadingState {
                                    upload_part_future,
                                    upload_size,
                                });
                                Poll::Pending
                            }
                        }
                    } else {
                        state.upload_state = UploadState::None;
                        Poll::Ready(Ok(()))
                    }
                },
                Poll::Ready(Err(e)) => {
                    state.upload_state = UploadState::Err(e.to_string());
                    return Poll::Ready(Err(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())));
                },
                Poll::Pending => {
                    Poll::Pending
                }
            }
        } else {
            Poll::Ready(Ok(()))
        }
    }

    fn poll_shutdown(
        self: Pin<&mut Self>,
        _: &mut Context<'_>
    ) -> Poll<Result<(), std::io::Error>> {
        // call flush
        Poll::Ready(Ok(()))
    }
}

#[async_trait]
impl IBackupChunkTargetProvider for S3ChunkTarget {
    async fn get_target_info(&self) -> Result<String> {
        Ok("aws s3".to_string())
    }

    fn get_target_url(&self) -> String {
        self.url.clone()
    }

    async fn get_account_session_info(&self) -> Result<String> {
        Ok(String::new())
    }

    async fn set_account_session_info(&self, _: &str) -> Result<()> {
        Ok(())
    }

    async fn is_chunk_exist(&self, chunk_id: &ChunkId) -> Result<(bool, u64)> {
        let key = chunk_id.to_string();
        
        match self.client.head_object()
            .bucket(&self.bucket)
            .key(&key)
            .send()
            .await
        {
            Ok(response) => {
                let size = response.content_length().unwrap_or(0);
                Ok((true, size as u64))
            },
            Err(err) => {
                if err.to_string().contains("NotFound") {
                    Ok((false, 0))
                } else {
                    Err(anyhow!("Failed to check object existence: {}", err))
                }
            }
        }
    }

    async fn link_chunkid(&self, target_chunk_id: &ChunkId, new_chunk_id: &ChunkId) -> BackupResult<()> {
        let target_key = target_chunk_id.to_string();
        let new_key = new_chunk_id.to_string();

        // 先获取源对象的元数据
        let head = self.client
            .head_object()
            .bucket(&self.bucket)
            .key(&target_key)
            .send()
            .await
            .map_err(|e| BuckyBackupError::Failed(format!("Failed to get source object metadata: {}", e)))?;

        // 构建新的元数据
        let metadata = head.metadata().cloned().unwrap_or_default();
        let mut target_metadata = metadata.clone();
        target_metadata.insert("link_target".to_string(), new_key.clone());

        // 更新源对象的元数据
        self.client
            .copy_object()
            .copy_source(format!("{}/{}", self.bucket, target_key))
            .bucket(&self.bucket)
            .key(&target_key)
            .metadata_directive(MetadataDirective::Replace)
            .set_metadata(Some(target_metadata))
            .send()
            .await
            .map_err(|e| BuckyBackupError::Failed(format!("Failed to update source metadata: {}", e)))?;


        let mut new_metadata = metadata;
        new_metadata.insert("link_target".to_string(), target_key.clone());
        // 复制对象并创建新的链接
        self.client
            .copy_object()
            .copy_source(format!("{}/{}", self.bucket, target_key))
            .bucket(&self.bucket)
            .key(new_key)
            .metadata_directive(MetadataDirective::Replace)
            .set_metadata(Some(new_metadata))
            .send()
            .await
            .map_err(|e| BuckyBackupError::Failed(format!("Failed to create link: {}", e)))?;

        Ok(())
    }

    async fn query_link_target(&self, source_chunk_id: &ChunkId)->BackupResult<Option<ChunkId>> {
        let key = source_chunk_id.to_string();
        let head = self.client
            .head_object()
            .bucket(&self.bucket)
            .key(&key)
            .send()
            .await
            .map_err(|e| BuckyBackupError::Failed(format!("Failed to get object head: {}", e)))?;
        Ok(head.metadata().and_then(|metadata| metadata.get("link_target"))
            .map(|target_key| ChunkId::new(target_key).unwrap()))
    }

    async fn open_chunk_reader_for_restore(&self, chunk_id: &ChunkId, offset:u64) -> BackupResult<ChunkReader> {
        let key = chunk_id.to_string();
        
        let head = self.client
            .head_object()
            .bucket(&self.bucket)
            .key(&key)
            .send()
            .await
            .map_err(|e| BuckyBackupError::Failed(format!("Failed to get object head: {}", e)))?;

        let size = head.content_length().unwrap_or(0) as u64;

        // 从指定的offset开始请求
        let response = self.client
            .get_object()
            .bucket(&self.bucket)
            .key(&key)
            .range(format!("bytes={}-{}", offset, size - 1))
            .send()
            .await
            .map_err(|e| BuckyBackupError::Failed(format!("Failed to get object content: {}", e)))?;
        
        let reader = response.body.into_async_read();
        Ok(Box::pin(reader))
    }

    async fn open_chunk_writer(&self, chunk_id: &ChunkId, _offset: u64, size: u64) -> BackupResult<(ChunkWriter,u64)> {
        let key = chunk_id.to_string();
        
        {
            // 先检查是否已有进行中的上传
            let mut states = self.upload_states.lock().unwrap();
            if let Some(_) = states.get(&key) {
                //    返回正在上传的错误
                return Err(BuckyBackupError::Failed(format!("Chunk is being uploaded")));
            }

            let state = MultipartUploadState::new(size);
            states.insert(key.clone(), state);
        }
        

        // 检查对象是否已存在
        let head_result = self.client
            .head_object()
            .bucket(&self.bucket)
            .key(&key)
            .send()
            .await;

        match head_result {
            Ok(head) => {
                // 如果对象存在且大小相等,返回错误
                if head.content_length() == Some(size as i64) {
                    return Err(BuckyBackupError::AlreadyDone(format!("Chunk already exists")));
                }
            },
            Err(err) => {
                // 如果是对象不存在的错误则继续,其他错误则返回
                if !err.to_string().contains("NotFound") {
                    return Err(BuckyBackupError::Failed(format!("Failed to check object existence: {}", err)));
                }
            }
        }

        // 如果没有现有上传，创建新的
        // 先查询是否有未完成的上传
        let list_uploads = self.client
            .list_multipart_uploads()
            .bucket(&self.bucket)
            .prefix(&key)
            .send()
            .await
            .map_err(|e| BuckyBackupError::Failed(format!("Failed to list multipart uploads: {}", e)))?;

        let existing_upload = list_uploads.uploads()
            .iter().find(|u| u.key() == Some(&key));

        let (upload_id, uploaded_size) = if let Some(upload) = existing_upload {
            // 如果存在未完成的上传,直接使用
            // 查询已上传的分片
            let parts = self.client
                .list_parts()
                .bucket(&self.bucket)
                .key(&key)
                .upload_id(upload.upload_id().unwrap_or_default())
                .send()
                .await
                .map_err(|e| BuckyBackupError::Failed(format!("Failed to list parts: {}", e)))?;
            // 找到最大的part num，生成下一个part num
            let (_max_part_number, uploaded_size) = parts.parts().iter().fold((0, 0), |(max_num, size), p| {
                (max_num.max(p.part_number().unwrap_or(0)), 
                 size + p.size().unwrap_or(0) as u64)
            });

            let upload_id = upload.upload_id.clone()
                .ok_or_else(|| BuckyBackupError::Failed("No upload ID received".to_string()))?;

            (upload_id, uploaded_size)
        } else {
            // 否则创建新的上传
            let create_upload = self.client
                .create_multipart_upload()
                .bucket(&self.bucket)
                .key(&key)
                .send()
                .await
                .map_err(|e| BuckyBackupError::Failed(format!("Failed to create multipart upload: {}", e)))?;

            let upload_id = create_upload.upload_id()
                .ok_or_else(|| BuckyBackupError::Failed("No upload ID received".to_string()))?
                .to_string();

            (upload_id, 0)
        };

        // 更新状态为已创建
        {
            let mut states = self.upload_states.lock().unwrap();
            if let Some(state) = states.get_mut(&key) {
                state.set_created(upload_id.clone());
            }
        }

        let writer = S3ChunkWriter {
            client: self.client.clone(),
            bucket: self.bucket.clone(),
            key,
            upload_id, 
            chunk_size: size,
            state: Mutex::new(WriterState {
                uploaded_size,
                part_limit: usize::min(S3ChunkWriter::part_size(), (size - uploaded_size) as usize),
                part_buffer: Vec::new(),
                upload_state: UploadState::None,
            }),
        };

        Ok((Box::pin(writer), uploaded_size))
    }

    async fn complete_chunk_writer(&self, chunk_id: &ChunkId) -> BackupResult<()> {
        let key = chunk_id.to_string();

        // get and remove upload id in states
        if let Some(upload_id) = {
            let states = self.upload_states.lock().unwrap();
            states.get(&key).map(|state| state.get_upload_id().unwrap().to_owned())
        } {
            // 获取已上传的分片
            let parts = self.client
                .list_parts()
                .bucket(&self.bucket)
                .key(&key)
                .upload_id(&upload_id)
                .send()
                .await
                .map_err(|e| BuckyBackupError::Failed(format!("Failed to list parts: {}", e)))?;

            let mut sorted_parts = parts.parts().to_vec();
            sorted_parts.sort_by_key(|part| part.part_number());

            // convert to completed part
            let completed_parts = sorted_parts.iter().map(|part| CompletedPart::builder()
                .part_number(part.part_number().unwrap_or(0))
                .e_tag(part.e_tag().unwrap_or_default())
                .build()
            ).collect::<Vec<_>>();

            let completed_upload = CompletedMultipartUpload::builder()
                .set_parts(Some(completed_parts))
                .build();

            self.client
                .complete_multipart_upload()
                .bucket(&self.bucket)
                .key(&key)
                .upload_id(&upload_id)
                .multipart_upload(completed_upload)
                .send()
                .await
                .map_err(|e| BuckyBackupError::Failed(format!("Failed to complete multipart upload: {}", e)))?;

            // 删除状态
            let mut states = self.upload_states.lock().unwrap();
            states.remove(&key);

            Ok(())
        } else {
            return Err(BuckyBackupError::Failed("No upload ID found".to_string()));
        }
    }
} 