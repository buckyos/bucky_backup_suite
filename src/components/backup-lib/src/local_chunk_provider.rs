#![allow(unused)]
use serde_json::Value;
use async_trait::async_trait;
use anyhow::Result;
use log::info;
use std::collections::HashMap;
use crate::provider::*;


//待备份的chunk都以文件的形式保存目录下
pub struct LocalDirChunkProvider {
    pub dir_path: String,
}

#[async_trait]
impl IBackupChunkSourceProvider for LocalDirChunkProvider {

    async fn get_source_info(&self) -> Result<Value> {
        unimplemented!()
    }

    fn get_source_url(&self)->String{
        self.dir_path.clone()
    }

    fn is_local(&self)->bool{
        true
    }

    async fn lock_for_backup(&self,source_url: &str)->Result<()>{
        info!("LocalDirChunkProvider do nothing at lock_for_backup: {}",source_url);
        Ok(())
    }

    async fn unlock_for_backup(&self,source_url: &str)->Result<()>{
        info!("LocalDirChunkProvider do nothing at unlock_for_backup: {}",source_url);
        Ok(())
    }

    async fn open_item(&self, item_id: &str)->Result<ItemReader>{
        unimplemented!()
    }

    async fn on_item_backuped(&self, item_id: &str)->Result<()>{
        info!("LocalDirChunkProvider do nothing at on_item_backuped: {}",item_id);
        Ok(())
    }

    async fn prepare_items(&self)->Result<Vec<BackupItem>>{
        unimplemented!()
    }
}


pub struct LocalChunkTargetProvider {
    pub dir_path: String,
}

#[async_trait]
impl IBackupChunkTargetProvider for LocalChunkTargetProvider {
    async fn get_target_info(&self) -> Result<String> {
        unimplemented!()
    }

    fn get_target_url(&self)->String{
        self.dir_path.clone()
    }

    async fn get_account_session_info(&self)->Result<String>{
        unimplemented!()
    }
    async fn set_account_session_info(&self, session_info: &str)->Result<()>{
        unimplemented!()
    }
    //fn get_max_chunk_size(&self)->Result<u64>;
    //返回Target上已经存在的Checkpoint列表()
    //async fn get_checkpoint_list(&self)->Result<Vec<String>>;

    //下面的接口将要成为通用的http based的chunk操作接口
    //async fn get_support_chunkid_types(&self)->Result<Vec<String>>;
    
    async fn is_chunk_exist(&self, chunk_id: &str)->Result<bool>{
        unimplemented!()
    }
    async fn get_chunk_state(&self, chunk_id: &str)->Result<String> {
        unimplemented!()
    }
    //查询多个chunk的状态
    async fn query_chunk_state_by_list(&self, chunk_list: &mut Vec<String>)->Result<()> {
        unimplemented!()
    }

    async fn put_chunklist(&self, chunk_list: HashMap<String, Vec<u8>>)->Result<()> {
        unimplemented!()
    }
    //上传一个完整的chunk,允许target自己决定怎么使用reader
    async fn put_chunk(&self, chunk_id: &str, offset: u64, chunk_data: &[u8])->Result<()> {
        unimplemented!()
    }
    //使用reader上传，允许target自己决定怎么使用reader
    async fn put_chunk_by_reader(&self, chunk_id: &str, chunk_reader: ItemReader)->Result<()> {
        unimplemented!()
    }
    //通过上传chunk diff文件来创建新chunk
    async fn patch_chunk(&self, chunk_id: &str, chunk_reader: ItemReader)->Result<()> {
        unimplemented!()
    }

    async fn remove_chunk(&self, chunk_list: Vec<String>)->Result<()> {
        unimplemented!()
    }
    //说明两个chunk id是同一个chunk.实现者可以自己决定是否校验
    //link成功后，查询target_chunk_id和new_chunk_id的状态，应该都是exist
    async fn link_chunkid(&self, target_chunk_id: &str, new_chunk_id: &str)->Result<()> {
        unimplemented!()
    }
}


