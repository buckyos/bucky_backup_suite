use std::{collections::HashSet, path::PathBuf, time::SystemTime};

use crate::{
    checkpoint::{DirChildType, FileStreamReader, StorageReader},
    error::BackupResult,
    meta::{
        CheckPointMetaEngine, DirectoryMeta, DirectoryMetaEngine, FileDiffChunk, FileDiffMeta,
        FileMeta, LinkMeta, LogAction, LogMetaEngine, StorageItem, StorageItemAttributes,
        StorageItemEngine, StorageItemField,
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
        name: root_name.clone(),
        attributes: root_dir_attr,
        service_meta: None,
        children: vec![],
    };

    let mut all_read_files = vec![(None, StorageItem::Dir(root_dir), root_name)]; // <parent-index, item, full-path>
    let mut next_dir_index = 0;

    while next_dir_index < all_read_files.len() {
        let parent_index = next_dir_index;
        let parent_full_path = all_read_files[next_dir_index].2.as_path();
        let current_item = &all_read_files[next_dir_index].1;
        next_dir_index += 1;
        if let StorageItem::Dir(_) = current_item {
        } else {
            continue;
        }

        let mut dir_reader = reader.read_dir(parent_full_path).await?;

        while let Some(child) = dir_reader.next().await? {
            let parent_full_path = all_read_files[next_dir_index].2.as_path();
            let child_name = child.path().to_path_buf();
            let full_path = parent_full_path.join(&child_name);
            let child_attr = reader.stat(&child_name).await?;
            let child_meta = match child {
                DirChildType::File(_) => {
                    let file_size = reader.file_size(&child_name).await?;
                    let hash = file_hash_from_reader(&mut FileStreamReader::new(
                        reader,
                        full_path.as_path(),
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

            all_read_files.push((Some(parent_index), child_meta, full_path));
        }
    }

    loop {
        let (parent_index, child_meta, _) = all_read_files
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

// meta = meta(reader) - meta(base_meta)
pub async fn meta_from_delta(
    base_meta: &StorageItemEngine,
    base_reader: &dyn StorageReader,
    reader: &dyn StorageReader,
) -> BackupResult<DirectoryMetaEngine> {
    let root_name = PathBuf::from("/");
    let root_dir_attr = reader.stat(&root_name).await?;
    let root_dir = DirectoryMetaEngine {
        name: root_name.clone(),
        attributes: root_dir_attr,
        service_meta: None,
        children: vec![],
    };

    // makesource there is a directory in base.
    let mut base_root_dir = DirectoryMetaEngine {
        name: root_name.clone(),
        attributes: StorageItemAttributes {
            create_time: SystemTime::UNIX_EPOCH,
            last_update_time: SystemTime::UNIX_EPOCH,
            owner: "".to_string(),
            group: "".to_string(),
            permissions: "".to_string(),
        },
        service_meta: None,
        children: vec![],
    };
    let base_meta = if let StorageItemEngine::Dir(base_dir) = base_meta {
        base_dir
    } else {
        base_root_dir.children.push(base_meta.clone());
        &base_root_dir
    };

    let mut all_read_files = vec![(None, StorageItem::Dir(root_dir), root_name)]; // <parent-index, item, full-path>
    let mut all_reader_dir_indexs = vec![(0, true)]; // <index-in-all_read_files, is-updated>
    let mut all_base_dirs = vec![Some(base_meta)];
    let mut next_dir_index = 0;

    while next_dir_index < all_reader_dir_indexs.len() {
        let current_base_dir = all_base_dirs[next_dir_index];
        let (parent_index, is_dir_attr_update) = all_reader_dir_indexs[next_dir_index];
        let parent_full_path = all_read_files[parent_index].2.clone();
        let current_item = &all_read_files[parent_index].1;
        next_dir_index += 1;
        let current_dir = if let StorageItem::Dir(current_dir) = current_item {
            current_dir
        } else {
            unreachable!("only directory can have children");
        };

        let mut dir_reader = reader.read_dir(parent_full_path.as_path()).await?;
        let mut children = HashSet::new();

        while let Some(child) = dir_reader.next().await? {
            let child_name = child.path().to_path_buf();
            let full_path = parent_full_path.join(&child_name);

            children.insert(child_name.clone());

            let base_child = current_base_dir.map_or(None, |dir| {
                dir.children
                    .iter()
                    .find(|c| c.name() == child_name.as_path())
            });

            let child_attr = reader.stat(&child_name).await?;

            // compare attributes
            let mut update_attr_log = None;
            if let Some(base_child) = base_child {
                if base_child.attributes() != &child_attr {
                    update_attr_log = Some(LogMetaEngine {
                        name: child_name.clone(),
                        attributes: child_attr.clone(),
                        service_meta: None,
                        action: LogAction::UpdateAttributes,
                    })
                }
            }

            let child_meta = match child {
                DirChildType::File(_) => {
                    let file_size = reader.file_size(&child_name).await?;
                    let hash = file_hash_from_reader(&mut FileStreamReader::new(
                        reader,
                        full_path.as_path(),
                        0,
                        1024 * 1024,
                    ))
                    .await?;
                    match base_child {
                        Some(base_child) => {
                            // TODO: move/copy/recovery from other directory.
                            if let StorageItemEngine::File(base_file) = base_child {
                                if base_file.hash == hash && base_file.size == file_size {
                                    // no update
                                    None
                                } else {
                                    // diff
                                    let diffs = diff_file_from_reader(
                                        &mut FileStreamReader::new(
                                            base_reader,
                                            full_path.as_path(),
                                            0,
                                            1024 * 1024,
                                        ),
                                        &mut FileStreamReader::new(
                                            reader,
                                            full_path.as_path(),
                                            0,
                                            1024 * 1024,
                                        ),
                                    )
                                    .await?;
                                    Some(StorageItemEngine::FileDiff(FileDiffMeta {
                                        name: child_name,
                                        attributes: child_attr,
                                        service_meta: None,
                                        hash,
                                        size: file_size,
                                        upload_bytes: 0,
                                        diff_chunks: diffs,
                                    }))
                                }
                            } else {
                                // new file
                                Some(StorageItemEngine::File(FileMeta {
                                    name: child_name,
                                    attributes: child_attr,
                                    service_meta: None,
                                    hash,
                                    size: file_size,
                                    upload_bytes: 0,
                                }))
                            }
                        }
                        None => {
                            // new file
                            Some(StorageItemEngine::File(FileMeta {
                                name: child_name,
                                attributes: child_attr,
                                service_meta: None,
                                hash,
                                size: file_size,
                                upload_bytes: 0,
                            }))
                        }
                    }
                }
                DirChildType::Dir(_) => {
                    let is_type_changed = match base_child {
                        Some(base_child) => {
                            if let StorageItemEngine::Dir(base_child) = base_child {
                                all_base_dirs.push(Some(base_child));
                                false
                            } else {
                                all_base_dirs.push(None);
                                true
                            }
                        }
                        None => {
                            all_base_dirs.push(None);
                            true
                        }
                    };
                    all_reader_dir_indexs.push((
                        all_read_files.len(),
                        is_type_changed || update_attr_log.is_some(),
                    ));
                    Some(StorageItemEngine::Dir(DirectoryMeta {
                        name: child_name,
                        attributes: child_attr,
                        service_meta: None,
                        children: vec![],
                    }))
                }
                DirChildType::Link(_) => {
                    let link_info = reader.read_link(&child_name).await?;
                    let mut is_updated = true;
                    if let Some(base_child) = base_child {
                        if let StorageItemEngine::Link(base_link) = base_child {
                            if base_link.is_hard == link_info.is_hard
                                && base_link.target == link_info.target
                            {
                                // no update
                                is_updated = false;
                            }
                        }
                    }

                    if is_updated {
                        Some(StorageItemEngine::Link(LinkMeta {
                            name: child_name,
                            attributes: child_attr,
                            service_meta: None,
                            is_hard: link_info.is_hard,
                            target: link_info.target,
                        }))
                    } else {
                        None
                    }
                }
            };

            // next_dir_index has +1 in the loop
            if let Some(child_meta) = child_meta {
                all_read_files.push((Some(next_dir_index - 1), child_meta, full_path));
            } else if let Some(update_attr_log) = update_attr_log {
                all_read_files.push((
                    Some(next_dir_index - 1),
                    StorageItemEngine::Log(update_attr_log),
                    full_path,
                ));
            }
        }

        if let Some(base_dir) = current_base_dir {
            for child in base_dir.children.iter() {
                // add LogAction::Remove for the `child`` not in `children`.
                if !children.contains(child.name()) {
                    all_read_files.push((
                        Some(next_dir_index - 1),
                        StorageItemEngine::Log(LogMetaEngine {
                            name: child.name().to_path_buf(),
                            attributes: child.attributes().clone(),
                            service_meta: None,
                            action: LogAction::Remove,
                        }),
                        parent_full_path.join(child.name()),
                    ));
                }
            }
        }
    }

    loop {
        let (parent_index, child_meta, _) = all_read_files
            .pop()
            .expect("root directory should be in the list");
        if let Some(parent_index) = parent_index {
            let parent_index = if parent_index == all_reader_dir_indexs.len() - 1 {
                let (parent_index, is_updated) =
                    all_reader_dir_indexs.pop().expect("dir should existed");
                if !is_updated {
                    continue;
                }
                parent_index
            } else {
                all_reader_dir_indexs[parent_index].1 = true;
                let (parent_index, _) = all_reader_dir_indexs[parent_index];
                parent_index
            };

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

pub async fn file_hash_from_reader<'a>(reader: &mut FileStreamReader<'a>) -> BackupResult<String> {
    unimplemented!()
}

pub async fn diff_file_from_reader<'a>(
    base_reader: &mut FileStreamReader<'a>,
    reader: &mut FileStreamReader<'a>,
) -> BackupResult<Vec<FileDiffChunk>> {
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
                        StorageItem::FileDiff(diff) => {
                            occupy_size +=
                                diff.diff_chunks.iter().map(|diff| diff.length).sum::<u64>();
                        }
                    }
                }
            }
            StorageItem::File(file) => {
                occupy_size += file.size;
            }
            StorageItem::Link(_) => {}
            StorageItem::Log(_) => {}
            StorageItem::FileDiff(diff) => {
                occupy_size += diff.diff_chunks.iter().map(|diff| diff.length).sum::<u64>()
            }
        }
    }

    occupy_size
}
