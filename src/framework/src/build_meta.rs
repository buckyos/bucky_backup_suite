use std::path::PathBuf;

use crate::{
    checkpoint::{DirChildType, FileStreamReader, StorageReader},
    error::BackupResult,
    meta::{
        CheckPointMeta, CheckPointMetaEngine, DirectoryMeta, DirectoryMetaEngine, FileMeta,
        LinkMeta, StorageItem, StorageItemEngine,
    },
};

pub async fn meta_from_reader(reader: &dyn StorageReader) -> BackupResult<DirectoryMetaEngine> {
    // Traverse all hierarchical subdirectories of reader. read-dir() in a breadth first manner and store them in the structure of CheckPointMeta.
    // For each directory, call read_dir() to get the list of files and subdirectories in the directory.
    // For each file, call read_file() to get the file content.
    // For each subdirectory, call read_dir() recursively.
    // For each link, call read_link() to get the link information.
    // For each file, call stat() to get the file attributes.
    // Return the CheckPointMeta structure.

    let root_name = PathBuf::from("/");
    let root_dir_attr = reader.stat(&root_name).await?;
    let root_dir = DirectoryMetaEngine {
        name: root_name,
        attributes: root_dir_attr,
        service_meta: None,
        children: vec![],
    };

    let mut all_read_files = vec![(None, StorageItem::Dir(root_dir))];
    let mut next_dir_index = 0;

    while next_dir_index < all_read_files.len() {
        let parent_index = next_dir_index;
        let current_item = &mut all_read_files[next_dir_index].1;
        next_dir_index += 1;
        let current_dir = if let StorageItem::Dir(current_dir) = current_item {
            current_dir
        } else {
            continue;
        };

        let mut dir_reader = reader.read_dir(&current_dir.name).await?;

        while let Some(child) = dir_reader.next().await? {
            let child_name = child.path().to_path_buf();
            let child_attr = reader.stat(&child_name).await?;
            let child_meta = match child {
                DirChildType::File(_) => {
                    let file_size = reader.file_size(&child_name).await?;
                    let hash = file_hash_from_reader(&mut FileStreamReader::new(
                        reader,
                        child_name.clone(),
                        0,
                        1024 * 1024,
                    ))
                    .await?;
                    StorageItemEngine::File(FileMeta {
                        name: child_name,
                        attributes: child_attr,
                        service_meta: None,
                        hash,
                        size: file_size,
                        upload_bytes: 0,
                    })
                }
                DirChildType::Dir(_) => StorageItemEngine::Dir(DirectoryMeta {
                    name: child_name,
                    attributes: child_attr,
                    service_meta: None,
                    children: vec![],
                }),
                DirChildType::Link(_) => {
                    let link_info = reader.read_link(&child_name).await?;
                    StorageItemEngine::Link(LinkMeta {
                        name: child_name,
                        attributes: child_attr,
                        service_meta: None,
                        is_hard: link_info.is_hard,
                        target: link_info.target,
                    })
                }
            };

            all_read_files.push((Some(parent_index), child_meta));
        }
    }

    loop {
        let (parent_index, child_meta) = all_read_files
            .pop()
            .expect("root directory should be in the list");
        if let Some(parent_index) = parent_index {
            let parent = &mut all_read_files[parent_index].1;
            if let StorageItem::Dir(parent_dir) = parent {
                parent_dir.children.push(child_meta);
            } else {
                unreachable!("only directory can have children");
            }
        } else if let StorageItem::Dir(child_meta) = child_meta {
            return Ok(child_meta);
        }
    }
}

// meta = meta(target_reader) - meta(source_reader)
pub async fn meta_from_delta(
    source_reader: &dyn StorageReader,
    target_reader: &dyn StorageReader,
) -> BackupResult<DirectoryMetaEngine> {
    unimplemented!()
}

pub async fn file_hash_from_reader<'a>(reader: &mut FileStreamReader<'a>) -> BackupResult<String> {
    unimplemented!()
}

pub fn estimate_occupy_size(meta: &CheckPointMetaEngine) -> u64 {
    // meta size
    let mut occupy_size = serde_json::to_string(meta).unwrap().len() as u64;
    // calculate the size of all files(Type: DirChildType::File) in the meta.
    // Traverse it as meta_from_reader

    let mut wait_read_dirs = vec![&meta.root];

    while !wait_read_dirs.is_empty() {
        let current_item = wait_read_dirs.remove(0);

        match current_item {
            StorageItem::Dir(dir) => {
                for child in dir.children.iter() {
                    match child {
                        StorageItem::Dir(_) => {
                            wait_read_dirs.push(child);
                        }
                        StorageItem::File(file) => {
                            occupy_size += file.size;
                        }
                        StorageItem::Link(_) => {}
                        StorageItem::Log(_) => {}
                    }
                }
            }
            StorageItem::File(file) => {
                occupy_size += file.size;
            }
            StorageItem::Link(_) => {}
            StorageItem::Log(_) => {}
        }
    }
}
