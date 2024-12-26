use async_trait::async_trait;
use buckyos_backup_lib::{IBackupChunkTargetProvider, ChunkId, Error as BackupError, ChunkReader};
use aws_sdk_s3::{Client, Config};
use aws_config::meta::region::RegionProviderChain;
use aws_config::BehaviorVersion;
use std::collections::HashMap;
use std::sync::Mutex;
use aws_sdk_s3::types::CompletedMultipartUpload;
use aws_sdk_s3::types::CompletedPart;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
enum AccountSession {
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

#[derive(Default)]
struct MultipartUploadState {
    upload_id: String,
    completed_parts: Vec<CompletedPart>,
    total_size: Option<u64>,
}

pub struct S3ChunkTarget {
    client: Client,
    bucket: String,
    prefix: String,
    upload_states: Mutex<HashMap<String, MultipartUploadState>>,
}

impl S3ChunkTarget {
    pub async fn new(bucket: String, prefix: String, region: Option<String>) -> Result<Self, BackupError> {
        // 使用环境变量配置创建客户端
        Self::new_with_session(bucket, prefix, region, AccountSession::Environment).await
    }

    pub async fn new_with_session(
        bucket: String, 
        prefix: String, 
        region: Option<String>,
        session: AccountSession,
    ) -> Result<Self, BackupError> {
        let region_provider = RegionProviderChain::first_try(region.map(aws_sdk_s3::Region::new))
            .or_default_provider();

        let config_builder = aws_config::defaults(BehaviorVersion::latest())
            .region(region_provider);

        let config = match session {
            AccountSession::Environment => config_builder.load().await,
            AccountSession::AccessKey { 
                access_key_id, 
                secret_access_key, 
                session_token 
            } => {
                let credentials_provider = aws_config::credentials::ProvideCredentials::provide_credentials(
                    &aws_config::credentials::SharedCredentialsProvider::new(
                        aws_config::Credentials::new(
                            access_key_id,
                            secret_access_key,
                            session_token,
                            None,
                            "s3-chunk-target",
                        )
                    )
                ).await.map_err(|e| BackupError::Provider(format!("Failed to create credentials: {}", e)))?;

                config_builder
                    .credentials_provider(credentials_provider)
                    .load()
                    .await
            }
        };

        let s3_config = Config::new(&config);
        let client = Client::from_conf(s3_config);

        Ok(Self {
            client,
            bucket,
            prefix,
            upload_states: Mutex::new(HashMap::new()),
        })
    }

    fn get_object_key(&self, chunk_id: &ChunkId) -> String {
        if self.prefix.is_empty() {
            chunk_id.to_string()
        } else {
            format!("{}/{}", self.prefix.trim_end_matches('/'), chunk_id)
        }
    }

    async fn ensure_multipart_upload(&self, chunk_id: &ChunkId, total_size: Option<u64>) -> Result<String, BackupError> {
        let key = self.get_object_key(chunk_id);
        let mut states = self.upload_states.lock().unwrap();
        
        if let Some(state) = states.get(&key) {
            return Ok(state.upload_id.clone());
        }

        // 创建新的分片上传
        let create_upload = self.client
            .create_multipart_upload()
            .bucket(&self.bucket)
            .key(&key)
            .send()
            .await
            .map_err(|e| BackupError::Provider(format!("Failed to create multipart upload: {}", e)))?;

        let upload_id = create_upload.upload_id()
            .ok_or_else(|| BackupError::Provider("No upload ID received".to_string()))?
            .to_string();

        let state = MultipartUploadState {
            upload_id: upload_id.clone(),
            completed_parts: Vec::new(),
            total_size,
        };

        states.insert(key, state);
        Ok(upload_id)
    }

    async fn try_complete_upload(&self, key: &str, chunk_size: Option<u64>) -> Result<bool, BackupError> {
        let mut states = self.upload_states.lock().unwrap();
        
        if let Some(state) = states.get(key) {
            let total_size = state.total_size.or(chunk_size);
            
            if let Some(expected_size) = total_size {
                // 计算已上传的大小
                let uploaded_size: u64 = state.completed_parts.iter()
                    .map(|part| (part.part_number() as u64) * 5 * 1024 * 1024)
                    .sum();

                // 如果已上传大小达到或超过预期大小，完成上传
                if uploaded_size >= expected_size {
                    let state = states.remove(key).unwrap();
                    
                    // 按分片号排序
                    let mut sorted_parts = state.completed_parts;
                    sorted_parts.sort_by_key(|part| part.part_number());

                    let completed_upload = CompletedMultipartUpload::builder()
                        .set_parts(Some(sorted_parts))
                        .build();

                    self.client
                        .complete_multipart_upload()
                        .bucket(&self.bucket)
                        .key(key)
                        .upload_id(&state.upload_id)
                        .multipart_upload(completed_upload)
                        .send()
                        .await
                        .map_err(|e| BackupError::Provider(format!("Failed to complete multipart upload: {}", e)))?;

                    return Ok(true);
                }
            }
        }
        
        Ok(false)
    }
}

#[async_trait]
impl IBackupChunkTargetProvider for S3ChunkTarget {
    async fn get_target_info(&self) -> Result<String, BackupError> {
        Ok(format!("s3://{}/{}", self.bucket, self.prefix))
    }

    fn get_target_url(&self) -> String {
        format!("s3://{}/{}", self.bucket, self.prefix)
    }

    async fn get_account_session_info(&self) -> Result<String, BackupError> {
        // 返回环境变量配置的序列化结果
        serde_json::to_string(&AccountSession::Environment)
            .map_err(|e| BackupError::Provider(format!("Failed to serialize session info: {}", e)))
    }

    async fn set_account_session_info(&self, session_info: &str) -> Result<(), BackupError> {
        // 解析会话信息
        let session: AccountSession = serde_json::from_str(session_info)
            .map_err(|e| BackupError::Provider(format!("Failed to parse session info: {}", e)))?;

        // 使用新的会话信息重新创建客户端
        let new_target = Self::new_with_session(
            self.bucket.clone(),
            self.prefix.clone(),
            None, // 保持现有区域设置
            session
        ).await?;

        // 更新客户端
        // 注意：这里可能需要处理正在进行的上传
        *self = new_target;

        Ok(())
    }

    async fn is_chunk_exist(&self, chunk_id: &ChunkId) -> Result<(bool, u64), BackupError> {
        let key = self.get_object_key(chunk_id);
        
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
                    Err(BackupError::Provider(format!("Failed to check object existence: {}", err)))
                }
            }
        }
    }

    async fn query_chunk_state_by_list(&self, chunk_list: &mut Vec<ChunkId>) -> Result<(), BackupError> {
        for chunk_id in chunk_list.iter_mut() {
            let (exists, size) = self.is_chunk_exist(chunk_id).await?;
            if exists {
                chunk_id.set_length(size);
            }
        }
        Ok(())
    }

    async fn put_chunklist(&self, chunk_list: HashMap<ChunkId, Vec<u8>>) -> Result<(), BackupError> {
        for (chunk_id, data) in chunk_list {
            self.put_chunk(&chunk_id, &data).await?;
        }
        Ok(())
    }

    async fn put_chunk(&self, chunk_id: &ChunkId, chunk_data: &[u8]) -> Result<(), BackupError> {
        let key = self.get_object_key(chunk_id);

        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(key)
            .body(chunk_data.to_vec().into())
            .send()
            .await
            .map_err(|e| BackupError::Provider(format!("Failed to put object to S3: {}", e)))?;

        Ok(())
    }

    async fn append_chunk_data(
        &self,
        chunk_id: &ChunkId,
        offset_from_begin: u64,
        chunk_data: &[u8],
        is_completed: bool,
        chunk_size: Option<u64>,
    ) -> Result<(), BackupError> {
        let key = self.get_object_key(chunk_id);

        // 如果是从头开始写且是完整数据，直接上传
        if offset_from_begin == 0 && is_completed {
            return self.put_chunk(chunk_id, chunk_data).await;
        }

        // 确保存在分片上传，并传入总大小信息
        let upload_id = self.ensure_multipart_upload(chunk_id, chunk_size).await?;

        // 计算当前分片号（S3要求分片号从1开始）
        let part_number = (offset_from_begin / (5 * 1024 * 1024) + 1) as i32;

        // 上传当前分片
        let upload_part_output = self.client
            .upload_part()
            .bucket(&self.bucket)
            .key(&key)
            .upload_id(&upload_id)
            .part_number(part_number)
            .body(chunk_data.to_vec().into())
            .send()
            .await
            .map_err(|e| BackupError::Provider(format!("Failed to upload part: {}", e)))?;

        // 记录已完成的分片
        let completed_part = CompletedPart::builder()
            .part_number(part_number)
            .e_tag(upload_part_output.e_tag.unwrap_or_default())
            .build();

        // 更新上传状态
        {
            let mut states = self.upload_states.lock().unwrap();
            if let Some(state) = states.get_mut(&key) {
                state.completed_parts.push(completed_part);
            }
        }

        // 尝试完成上传（如果所有分片都已上传）
        if is_completed || self.try_complete_upload(&key, chunk_size).await? {
            // 上传已完成，无需额外操作
            return Ok(());
        }

        Ok(())
    }

    async fn link_chunkid(&self, target_chunk_id: &ChunkId, new_chunk_id: &ChunkId) -> Result<(), BackupError> {
        let target_key = self.get_object_key(target_chunk_id);
        let new_key = self.get_object_key(new_chunk_id);

        // 使用 S3 的复制功能创建链接
        self.client
            .copy_object()
            .copy_source(format!("{}/{}", self.bucket, target_key))
            .bucket(&self.bucket)
            .key(new_key)
            .send()
            .await
            .map_err(|e| BackupError::Provider(format!("Failed to link chunks: {}", e)))?;

        Ok(())
    }

    async fn open_chunk_reader_for_restore(&self, chunk_id: &ChunkId, _quick_hash: Option<ChunkId>) -> Result<ChunkReader, BackupError> {
        let key = self.get_object_key(chunk_id);
        
        // 获取对象大小
        let head = self.client
            .head_object()
            .bucket(&self.bucket)
            .key(&key)
            .send()
            .await
            .map_err(|e| BackupError::Provider(format!("Failed to get object head: {}", e)))?;

        let size = head.content_length().unwrap_or(0) as u64;
        
        // 创建并返回 ChunkReader
        // 注意：这里需要 ChunkReader 的具体实现
        todo!("Implement ChunkReader creation")
    }
} 