# 备份扇区格式设计

当备份任务的target是存储单元很大的写入不可修改的存储系统时（比如DMCX），首先从source生成写入扇区，之后将扇区提交到target；扇区的格式应当实现以下需求：
+ 对target的能力只做如下约束：
    + 单次写入，不可附加，不可修改
    + 可以遍历所有已写入内容
    + 可以任意偏移和大小读出
+ 扇区支持加密和校验，当target为不可信的公网节点时，为了保证隐私和安全，应当保证只有源可以从扇区还原，并且还原时可以校验内容是否正确；
+ 元数据应当嵌入到扇区中以支持零知识的本地还原；
+ 对target有较好的空间性能：
    + 支持增量备份
    + 支持备份内容压缩
    + 支持合并多次备份到一个扇区
+ 备份和还原过程应当有较好的空间性能：
    + 备份过程中产生的临时磁盘占用，相对于source的大小应当是O(1)的，较大的备份内容可以分割到多个扇区中去，顺序生成确定大小的扇区之后写入target，本地可以删除该扇区；
    + 备份过程中对备份内容加密和压缩过程是原地的，不需要额外的空间；备份和还原过程中的解密和解压缩过程也应当是原地的，不需要额外的空间；
    + 零知识还原过程可以只重建元数据，而不需要重建备份数据；


## 结构

### 整体
+ magic

    确定的4字节，target并非只用于备份，在零知识还原时，可以通过遍历target已写入内容匹配该magic确定是否为备份扇区；

+ sector header

    扇区头，无论是否加密和压缩，扇区头部分都是明文原文

+ chunk
    一到多个check point box；
    check point表示在某个时间点进行的一次备份，该次check point产生的备份内容被封装到chunk中；
    当check point比较大或者chunk容量不足时，可以拆分到多个box嵌入到不同的chunk中；
    当有多个较小的check point时，每个check point生成一个box，多个box嵌入到同一个chunk中；


### sector header
+ version
    
    2字节版本号，用于实现扇区版本兼容性

+ flags

    32bit的开关值，当前可有
    + 加密： 是否经过加密
    + 签名： 是否包含签名

+ [public key]

    加密或签名时存在该字段，写入用于加密和签名的非对称密钥对的公钥；

+ [enrypt key]

    加密时存在该字段；
    加密过程应当首先产生非对称密钥对和对称密钥，使用对称密钥对内容加密，使用共钥对对称密钥加密后写入该字段；

+ chunk id

    sector装载的chunk id；

+ [signature]

    签名时存在该字段，对扇区除本字段之外的所有内容做摘要之后，使用私钥对摘要加密之后写入该字段；
    如果chunk id本身能校验chunk 内容，就可以只签名 chunk id； 如果target的共识本身支持校验内容（比如dmcx），就可以不需要sector 签名；

### check point box
+ uuid

    为每一个check point生成uuid，如果check point被分拆到多个chunk时，这些box有相同的uuid；
+ offset in check point

    该chunk在check point中的起始偏移，如果check point没有分拆，该值为0；

+ box's length
    
    chunk content的长度, 这里加一个细节，用第一个bit标识是不是尾chunk

+ box content

    check point并不直接写入扇区，而是通过封装到chunk之后写入；如果check point没有分拆，chunk content就是check point；如果check point分拆到多个chunk， 将这些有相同uuid的box按照 offset in check point的顺序合并之后才是check point；


## check point 
+ flags

    32bit的开关值，当前可有：

    + 增量： 是否是基于更早的某个check point的增量备份
+ timestamp

    生成该check point的本地时间戳

+ [based on check point]

    当check point是增量备份时存在该字段，写入基于的check point的uuid

+ actions
    
    生成该备份的action序列


## action
+ flags
    32bit的开关值，当前可有：

    + 压缩： action content是否经过压缩
+ relative path

    该action操作的相对路径
+ type

    action的类型
    
+ offset in action

    考虑可以快速还原meta信息，但是action content在非常大或者跨chunk时，特别时加上压缩之后，难以提前生成整个action 的length；
    当生成完整action时再去修正已经写入的chunk字段也会打破O（1）临时空间占用的约定，所以当出现action跨越box边界时，还是需要冗余的action meta；
    这里跟box 的offset in checkpoint 一样

+ content length

    action content的长度（如果经过压缩，是指压缩后的长度）;
   

+ action content

    不同的action type有不同的content格式，有些action是嵌入的结构化数据，有些是非结构化内容；
    比如 action是新增一个文件， 并且压缩， content就是压缩后的新增文件内容；


## 结构上的问题
1. 为了能够从chunk target上最快速读取特定action的内容（增量备份和还原时），action之间不能有依赖，所以这里设计的是对每个action的内容单独压缩；如果是比较大的文件，问题不大，但是如果是很多个小文件，压缩率会相对整个box压缩差很多； 这部分可以设计更多的策略，实现空间和时间性能上的平衡；

2. 对单个巨大文件的处理过程不太清楚，假如一个单文件超过32G；比如dmc设定的sector大小上上限是32G，表示的是 source在生成chunk时，如果chunk文件达到32G，应当中断等待 dmc target打包chunk到sector完成并且上传到dmc网络完成，才能继续生成下一个chunk，否则就没法保证本地空间O（1）的约定，而且封装sector和上传的时间也会是比较长的； 
不太清楚文件系统锁的实现和限制，如果是小文件，中断的实现可以是忽略后面的action，合并到下一次check point；如果是单个大文件，是不能分离出多个action的，但是如果等上传的过程中，文件内容改变（系统文件锁能不能锁那么久？），也没办法回退正在上传的chunk，这种情况要怎么处理；

3. aes算法的选择，用ecb的好处是可以实现在sector随机读，并且加解密并行，也可以中断和恢复加解密过程，如果chunk本身压缩过，安全性问题大不大？ 
    用cbc的话，一个aes的session的粒度怎么设计，一个chunk，一个sector等等；如果chunk 加密之后写进sector，之后sector写道dmc，本地删掉sector，如果是chunk 粒度的依赖（比如diff的时候），就要先把整个chunk都拉回来解密；


## 定义结构
```rust

// structures in sector file
struct ActionHeader {
    pub action_type: ActionType,
    pub path: String,
    pub offset_in_chunk: u64,
    pub length: u64,
}

struct CheckPointBoxHeader {
    pub checkpoint_uuid: Uuid,
    pub offset_in_checkpoint: u64,
    pub length: u64, 
    pub is_tail: bool,
}


struct SectorHeader {
    pub sector_id: SectorId,
    pub chunk_id: ChunkId,  
    pub chunk_start: u64, 
    pub flags: u32,
    pub public_key: PublicKey,
    pub encrypt_key: AesKey
}

// meta info in local cache
struct ActionMeta {
    pub action_type: ActionType, 
    pub path: String, 
    // 记录action在chunk 中的偏移和大小
    pub contents: Vec<(ChunkId, u64, u64)>, 
}


struct CheckPointMeta {
    pub info: CheckPointInfo,
    pub actions: Vec<ActionMeta>,
}


struct CheckPointInfo {
    pub uuid: Uuid,
    pub rely_on: Option<Uuid>,
    pub timestamp: Timestamp,
}

```


### 备份过程
```rust

impl DirChunkSource {
    // 系统事件驱动，目录变化时触发，生成checkpoint 封装到chunk
    fn triger_checkpoint(&self, actions: Vec<Action>) {
        // 还有没写完的chunk
        let chunk = {
            if let Some(pending_chunk) = self.pending_chunk {
                pending_chunk
            } else {
                let chunk = Chunk::new();
                self.pending_chunk = Some(chunk);
                chunk
            }
        };
        let checkpoint_header = CheckPointInfo {
            uuid: Uuid::new_v4(),
            rely_on: None,
            timestamp: Timestamp::now(),
        };

        for action in actions {
            if chunk.size() > MAX_CHUNK_SIZE {
                // 等到check被target消费掉才能继续
                wait()
            } else {
                // chunk 大小未超过限制，继续写入
                for action in actions {
                    io::copy(chunk, &action.action_type);
                    io::copy(chunk, &action.path);
                    // 压缩 action内容
                    compress::compress(&action.content);
                    
                    if chunk.size() + action.content.len() > MAX_CHUNK_SIZE {
                        // chunk 内容超过限制，等待check被target消费掉
                        wait()
                    }
                    io::copy(chunk, &action.content);
                }
            }
        }
    }
}


impl Dmcx {
    // chunk target 在备份过程中，打包source生成的chunk，上传到dmcx网络
    fn backup_chunk(&self, chunk: &ChunkId) {
        let sector_tmp_path = "/tmp/sector";
        let mut sector_tmp = File::open(sector_tmp_path);

        let pk, sk = gen_rsa_keypair();
        let aes_key = gen_aes_key();

        let mut md = MessageDigest::sha256();

        // write sector header
        // write magic
        io::copy(sector_tmp, &SECTOR_MAGIC.to_be_bytes()).unwrap();
        md.update(&SECTOR_MAGIC.to_be_bytes());
        // write version
        io::copy(sector_tmp, &SECTOR_VERSION_0.to_be_bytes()).unwrap();
        md.update(&SECTOR_VERSION_0.to_be_bytes());
        // write flags
        io::copy(sector_tmp, &(SECTOR_FLAG_ENCRYPT|SECTOR_FLAG_SIGN).to_be_bytes()).unwrap();
        md.update(&(SECTOR_FLAG_ENCRYPT|SECTOR_FLAG_SIGN).to_be_bytes());
        // write public key
        io::copy(sector_tmp, &pk.to_bytes());
        md.update(&pk.to_bytes());
        
        let encrypt_key = pk.encrypt(&aes_key);
        // write encrypted key
        io::copy(sector_tmp, &encrypt_key.to_bytes());
        md.update(&encrypt_key.to_bytes());

        // write chunk id
        io::copy(sector_tmp, &chunk.to_bytes());
        md.update(&chunk.to_bytes());


        // placeholder for signature
        io::skip(sector_tmp, 32);
        
        // write content
        let buffer = read_chunk(chunk);
        io::copy(sector_tmp, &buffer);

        // 省略把sector写到dmcx的过程...
    }
    
}

```

### 还原过程
```rust
trait ChunkTarget {
    // 列出所有chunk
    fn list(&self) -> Vec<ChunkId>;
    // 支持随机读chunk数据
    fn read(&self, chunk: ChunkId, offset: u64, length: u64) -> Vec<u8>;
}


// dmc实现chunk target
impl ChunkTarget for Dmcx {
    fn list(&self) -> Vec<ChunkId> {
        if self.local_cache.is_updated() {
            self.local_cache.list_chunks()
        } else {
            let mut chunks = Vec::new();
            // 遍历dmcx上的所有写入，读取头部magic，判断是否是sector
            for sector in self.dmcx_client.list_sectors() {
                let mut offset = 0;
                let mut magic = [0u8; 4];
                io::copy(sector, &magic).unwrap();
                offset += 4;
                if u32::from_be_bytes(magic) == SECTOR_MAGIC {
                    chunks.push(sector.uuid());
                }
                let mut chunk_id = [0u8; 16];
                io::copy(sector, &chunk_id).unwrap();
                offset += 16;
                let chunk_id = ChunkId::from_bytes(&chunk_id);
                chunks.push(chunk_id);

                // 省略之后读共钥，解密key
                // ...

                let aes_key;
                
                self.local_cache.add_chunk(chunk_id, SectorHeader {
                    sector_id: sector.uuid(),
                    chunk_id,
                    chunk_start: offset,
                    encrypt_key: aes_key,
                });
            }
            chunks
        }
    }

    fn read(&self, chunk: ChunkId, offset: u64, length: u64) -> Vec<u8> {
        if let Some(sector_info) = self.local_cache.chunk_of(&chunk) {
            let sector = self.dmcx_client.sector_of(sector_info.sector_id);
            let mut buffer = vec![0u8; length as usize];
            
            // 从dmcx读sector，并解密
            setor.seek(io::SeekFrom::Start(offset + sector_info.chunk_start));
            io::copy(sector, &buffer); 
            sector_info.encrypt_key.decrypt(&mut buffer);
            buffer
        }
    }
}


impl DirChunkSource {
    // 从 chunk target 还原元数据
    fn restor_meta_from_chunk_target(&self, target: impl ChunkTarget) {
        for chunk_id in target.list() {
            let mut offset = 0;
            let mut cur_checkpoint = None;
            loop {
                // 读出 box header
                let buffer = target.read(chunk_id, offset, size_of(CheckPointBoxHeader));
                offset += size_of(CheckPointBoxHeader);
                if let Some(checkpoint_box_info) = CheckPointBoxHeader::from_bytes(&buffer) {
                    // 第一个box，可以读取checkpoint header
                    if checkpoint_box_info.offset_in_checkpoint == 0 {
                        assert!(cur_checkpoint.is_none());

                        let buffer = target.read(chunk_id, offset, size_of(CheckPointInfo));
                        offset += size_of(CheckPointInfo);
                        let checkpoint_info = CheckPointInfo::from_bytes(&buffer).unwrap();

                        cur_checkpoint = Some(CheckPointMeta {
                            uuid: checkpoint_info.checkpoint_uuid,
                            rely_on: checkpoint_info.rely_on,
                            timestamp: checkpoint_info.timestamp,
                            actions: Vec::new(),
                        });
                    } else {
                        // 省略header被分割的实现...
                        // 继续当前的check point
                        assert_eq!(cur_checkpoint.as_ref().unwrap().info.uuid == checkpoint_box_info.checkpoint_uuid);
                    }

                    loop {
                        // 顺序读出action header
                        let buffer = target.read(chunk_id, offset, size_of(ActionHeader));
                        offset += size_of(ActionHeader);
                        if let Some(action_header) = ActionHeader::from_bytes(&buffer) {
                            if action_header.offset_in_chunk == 0 {
                                // 第一个action header， 新增action meta
                                cur_checkpoint.as_mut().unwrap().actions.push(ActionMeta {
                                    action_type: action_header.action_type, 
                                    path: action_header.path,
                                    contents: Vec::new([(chunk_id, action_header.offset_in_chunk, action_header.length)]),
                                });
                            } else {
                                // 当前action meta添加
                                let mut cur_action = cur_checkpoint.as_mut().unwrap().actions.last_mut().unwrap();
                                assert_eq!(cur_action.action_type, action_header.action_type);
                                assert_eq!(cur_action.path, action_header.path);
                                cur_action.contents.push((chunk_id, action_header.offset_in_chunk, action_header.length));
                            }
                        } else {
                            break;
                        }
                    }   


                    // 不合法的情况：没有tail开始了下一个checkpoint，offset不连续等等，都应当视作check point 不完整忽略
                    if checkpoint_box_info.is_tail {
                        // 一个checkpoint读取完毕
                        self.local_cache.add_checkpoint_meta(cur_checkpoint.unwrap());
                        cur_checkpoint = None;
                    } 

                   
                } else {
                    break;
                }
            }
        
        }
    }

    // 从 本地缓存的 meta 和 chunk target还原
    fn restore_from_chunk_target(&self, target: impl ChunkTarget, checkpoint_id: &Uuid) {
        // 如果有增量依赖应当返回依赖链
        let checkpoint_meta_list = self.local_cache.checkpoint_meta_of(checkpoint_id);
        let target_checkpoint = checkpoint_meta_list.last().unwrap();
        for action_meta in target_checkpoint.actions {
            // 根据action meta 去读内容
            for (chunk_id, offset, length) in action_meta.contents {
                let buffer = target.read(chunk_id, offset, length);
                // 只有content部分压缩过
                compress::decompress(buffer);
                // 省略应用内容到本地目录的部分...
                for rely_checkpoint in &checkpoint_meta_list[0..-2] {
                    let rely_action = rely_checkpoint.actions.as_iter().find(|action| action.path == action_meta.path).unwrap();
                    for (rely_chunk_id, rely_offset, rely_length) in rely_action.contents {
                        let rely_buffer = target.read(rely_chunk_id, rely_offset, rely_length);
                        // 要和依赖通过 difference一起还原内容...
                        // 只有content部分压缩过
                        compress::decompress(rely_buffer);
                    }
                }
            }
        }
    }       
}
```



