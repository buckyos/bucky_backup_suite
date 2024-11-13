#![allow(unused)]
use crate::provider::*;
use anyhow::Result;

pub struct HashContext {
    hash_type:String,
}

pub fn build_full_hash_context(hash_type:Option<&str>)->HashContext {
    HashContext{
        hash_type:hash_type.unwrap_or("sha256").to_string(),
    }
}

impl HashContext {
    pub fn update(&self, data: &[u8]) {
        unimplemented!()
    }

    pub fn get_hash(&self)->String {
        unimplemented!()
    }
}

pub fn is_all_item_have_chunk_id(item_list: &Vec<BackupItem>)->bool {
    item_list.iter().all(|item| item.chunk_id.is_some())
}

pub fn item_list_to_chunk_id_list(item_list: &Vec<BackupItem>)->Vec<String> {
    item_list.iter().map(|item| item.chunk_id.as_ref().unwrap().clone()).collect()
}


pub async fn calculate_quick_hash(item_reader: &ItemReader)->Result<String> {
    unimplemented!()
}

pub async fn calculate_full_hash(item_reader: &ItemReader)->Result<String> {
    unimplemented!()
}

pub async fn calculate_full_hash_by_content(content: &Vec<u8>)->Result<String> {
    unimplemented!()
}

