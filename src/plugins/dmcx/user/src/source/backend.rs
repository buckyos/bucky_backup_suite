use log::*;
use rand::Rng;
use std::{
    collections::HashMap, convert::TryInto, sync::Arc
};
use async_std::{
    fs, io::prelude::*, stream::StreamExt, sync::RwLock, task};
use serde::{Serialize, Deserialize};
use sqlx::Row;
use tide::{Request, Response, Body, http::Url};
use dmc_tools_common::*;
#[cfg(feature = "sqlite")]
use sqlx_sqlite as sqlx_types;
#[cfg(feature = "mysql")]
use sqlx_mysql as sqlx_types;
use crate::{
    journal::*
};
use super::{
    types::*
};



#[derive(sqlx::FromRow)]
struct SourceStateRow {
    source_id: sqlx_types::U64, 
    source_url: String, 
    length: sqlx_types::U64,
    update_at: sqlx_types::U64, 
    state_code: sqlx_types::U8, 
    merkle_stub: Option<Vec<u8>>,   
    merkle_options: Vec<u8>, 
    stub_options: Vec<u8>
}

impl TryInto<DataSource> for SourceStateRow {
    type Error = DmcError;
    fn try_into(self) -> DmcResult<DataSource> {
        let merkle_stub = if let Some(value) = self.merkle_stub {
            serde_json::from_slice(&value)?
        } else {
            None
        };
        Ok(DataSource {
            source_id: self.source_id as u64,  
            length: self.length as u64, 
            update_at: self.update_at as u64, 
            source_url: self.source_url, 
            merkle_stub, 
            state: SourceState::try_from(self.state_code as u8)?
        })
    }
}

impl From<&CreateSourceOptions> for SourceStateRow {
    fn from(options: &CreateSourceOptions) -> Self {
        Self {
            source_id: 0, 
            source_url: options.source_url.clone(),  
            length: options.length as sqlx_types::U64,
            update_at: 0, 
            state_code: SourceState::PreparingLeafStub as u8 as sqlx_types::U8, 
            merkle_stub: None,   
            merkle_options: serde_json::to_vec(&options.merkle).unwrap(), 
            stub_options: serde_json::to_vec(&options.stub).unwrap()
        }
    }
}

impl TryInto<CreateSourceOptions> for &SourceStateRow {
    type Error = DmcError;
    fn try_into(self) -> DmcResult<CreateSourceOptions> {
        Ok(CreateSourceOptions {
            source_url: self.source_url.clone(),  
            length: self.length as u64, 
            merkle: serde_json::from_slice(&self.merkle_options)?,
            stub: serde_json::from_slice(&self.stub_options)?
        })
    }
}

#[derive(sqlx::FromRow)]
struct SourceStubRow {
    source_id: sqlx_types::U64, 
    index: sqlx_types::U32, 
    offset: sqlx_types::U64, 
    length: sqlx_types::U16, 
    content: Vec<u8>
}

impl Into<SourceStub> for SourceStubRow {
    fn into(self) -> SourceStub {
        SourceStub {
            source_id: self.source_id as u64, 
            index: self.index as u32, 
            offset: self.offset as u64, 
            length: self.length as u16, 
            content: DmcData::from(self.content)
        } 
    }
}

#[derive(sqlx::FromRow)]
struct MerkleBlockStubRow {
    source_id: sqlx_types::U64, 
    index: sqlx_types::U32, 
    content: Vec<u8>
}

impl Into<MerkleBlockStub> for MerkleBlockStubRow {
    fn into(self) -> MerkleBlockStub {
        MerkleBlockStub {
            source_id: self.source_id as u64, 
            index: self.index as u32, 
            content: self.content
        } 
    }
}


struct ServerImpl {
    sql_pool: sqlx_types::SqlPool,  
    journal: JournalServer, 
    source_url_parsers: RwLock<HashMap<String, Arc<Box<dyn SourceUrlParser>>>>,
}


#[derive(Clone)]
pub struct SourceServer(Arc<ServerImpl>);

impl std::fmt::Display for SourceServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "UserSourceServer {{process_id={}}}", std::process::id())
    }
}


pub trait ReadSeek: Read + Seek + Unpin + Send + Sync {}

impl<T: ReadSeek> ReadSeek for Box<T> {}


#[async_trait::async_trait]
pub trait SourceUrlParser: Send + Sync {
    async fn parse(&self, url: &Url) -> DmcResult<(Box<dyn ReadSeek>, u64)>;
}

pub trait AggregateSourceServer {
    fn source_server(&self) -> &SourceServer;
}

impl SourceServer {
    pub async fn listen<T: 'static + AggregateSourceServer + Clone + Send + Sync>(http_server: &mut tide::Server<T>) -> DmcResult<()> {
        http_server.at("/source").post(|mut req: Request<T>| async move {
            let options = req.body_json().await?;
            let mut resp = Response::new(200);
            resp.set_body(Body::from_json(&req.state().source_server().create(options).await?)?);
            Ok(resp)
        });
    
        http_server.at("/source").get(|req: Request<T>| async move {
            let mut query = SourceFilterAndNavigator::default();
            for (key, value) in req.url().query_pairs() {
                match &*key {
                    "source_id" => {
                        query.filter.source_id = Some(u64::from_str_radix(&*value, 10)?);
                    },
                    "source_url" => {
                        query.filter.source_url = Some((&*value).to_owned());
                    },
                    "page_size" => {
                        query.navigator.page_size = usize::from_str_radix(&*value, 10)?;
                    },
                    "page_index" => {
                        query.navigator.page_index = usize::from_str_radix(&*value, 10)?;
                    }, 
                    _ => {}
                }
            }
            // let query = req.query::<SourceFilterAndNavigator>()?;
            
            let mut resp = Response::new(200);
            let sources = req.state().source_server().get(query.filter, query.navigator).await?;
            resp.set_body(Body::from_json(&sources)?);
            Ok(resp)
        });

        http_server.at("/source/stub").get(|req: Request<T>| async move {
            let mut source_id = None;
            for (key, value) in req.url().query_pairs() {
                match &*key {
                    "source_id" => {
                        source_id = Some(u64::from_str_radix(&*value, 10)?);
                    }, 
                    _ => {}
                }
            }
            // let query = req.query::<SourceFilterAndNavigator>()?;
            let mut resp = Response::new(200);
            if let Some(source_id) = source_id {
                let stub = req.state().source_server().random_stub(source_id).await?;
                resp.set_body(Body::from_json(&stub)?);
                Ok(resp)
            } else {
                unreachable!()
            }
        });


        // http_server.at("/source/detail").get(|req: Request<T>| async move {
        //     let param = req.query::<QuerySource>()?;

        //     let detail = req.state().source_server().get_source_detail(param.source_id).await?;
        //     Ok(Response::from(Body::from_json(&detail)?))
        // });


        Ok(())
    }

    pub async fn with_sql_pool(sql_pool: sqlx_types::SqlPool, journal: JournalServer) -> DmcResult<Self> {
        Ok(Self(Arc::new(ServerImpl {
                sql_pool,  
                journal,
                source_url_parsers: RwLock::new(HashMap::new())
            })
        ))
    }

    pub async fn init(&self) -> DmcResult<()> {
        let _ = sqlx::query(&Self::sql_create_table_source_state()).execute(&self.0.sql_pool).await
            .map_err(|err| dmc_err!(DmcErrorCode::Failed, "{} init failed, err={}", self, err))?;
        let _ = sqlx::query(&Self::sql_create_table_merkle_stub()).execute(&self.0.sql_pool).await
            .map_err(|err| dmc_err!(DmcErrorCode::Failed, "{} init failed, err={}", self, err))?;
        let _ = sqlx::query(&Self::sql_create_table_source_stub()).execute(&self.0.sql_pool).await
            .map_err(|err| dmc_err!(DmcErrorCode::Failed, "{} init failed, err={}", self, err))?;
        Ok(())
    }

    pub async fn register_source_url_parser<T: 'static + SourceUrlParser>(&self, scheme: &str, parser: T) -> DmcResult<&Self> {
        let mut source_url_parsers = self.0.source_url_parsers.write().await;
        if source_url_parsers.contains_key(scheme) {
            return Err(dmc_err!(DmcErrorCode::AlreadyExists, "scheme {} already exists", scheme));
        }
        source_url_parsers.insert(scheme.to_owned(), Arc::new(Box::new(parser)));
        Ok(self)
    }


    async fn with_default_source_url_parsers(self) -> Self {
        struct FileSourceUrlParser;

        #[async_trait::async_trait]
        impl SourceUrlParser for FileSourceUrlParser {
            async fn parse(&self, url: &Url) -> DmcResult<(Box<dyn ReadSeek>, u64)> {
                let source_path;
                #[cfg(target_os = "windows")] {
                    source_path = &url.path()[1..];
                } 
                #[cfg(not(target_os = "windows"))] {
                    source_path = url.path().clone();
                }
        
                let source_file = fs::File::open(source_path).await?;
                let source_length = source_file.metadata().await?.len();

                impl ReadSeek for fs::File {}
                Ok((Box::new(source_file), source_length))
            }
        }

        self.register_source_url_parser("file", FileSourceUrlParser).await.unwrap();
        self
    }

    pub async fn reset(self) -> DmcResult<Self> {
        self.init().await?;
        let mut trans = self.sql_pool().begin().await?;
        let _ = sqlx::query("DELETE FROM source_state WHERE source_id > 0").execute(&mut trans).await?;
        let _ = sqlx::query("DELETE FROM merkle_stub WHERE source_id > 0").execute(&mut trans).await?;
        let _ = sqlx::query("DELETE FROM source_stub WHERE source_id > 0").execute(&mut trans).await?;
        trans.commit().await?;
        Ok(self)
    }
}



impl SourceServer {
    fn journal(&self) -> &JournalServer {
        &self.0.journal
    }

    fn sql_pool(&self) -> &sqlx_types::SqlPool {
        &self.0.sql_pool
    }

    #[cfg(feature = "mysql")]
    fn sql_create_table_source_state() -> &'static str {
        r#"CREATE TABLE IF NOT EXISTS `source_state` (
            `source_id` BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
            `source_url` VARCHAR(512) NOT NULL,  
            `length` BIGINT UNSIGNED NOT NULL,
            `state_code` TINYINT UNSIGNED NOT NULL, 
            `merkle_stub` BLOB,  
            `merkle_options` BLOB, 
            `stub_options` BLOB NOT NULL,
            `update_at` BIGINT UNSIGNED NOT NULL, 
            `process_id` INT UNSIGNED, 
            INDEX (`state_code`)
        ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;"#
    }

    #[cfg(feature = "sqlite")]
    fn sql_create_table_source_state() -> &'static str {
        r#"CREATE TABLE IF NOT EXISTS source_state (
            source_id INTEGER PRIMARY KEY AUTOINCREMENT,
            source_url TEXT NOT NULL,
            length  NOT NULL,
            state_code INTEGER NOT NULL, 
            merkle_stub BLOB,  
            merkle_options BLOB, 
            stub_options BLOB NOT NULL,
            update_at INTEGER NOT NULL,
            process_id INTEGER
        );
        CREATE INDEX IF NOT EXISTS state_code ON source_state (state_code);
        CREATE INDEX IF NOT EXISTS source_url ON source_url (source_url);
        "#
    }

    #[cfg(feature = "mysql")]
    fn sql_create_table_merkle_stub() -> &'static str {
        r#"CREATE TABLE IF NOT EXISTS `block_stub` (
            `source_id` BIGINT UNSIGNED NOT NULL, 
            `index` INT UNSIGNED NOT NULL, 
            `hash` BLOB, 
            INDEX (`source_id`),
            UNIQUE INDEX `source_index` (`source_id`, `index`)
        ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;"#
    }

    #[cfg(feature = "sqlite")]
    fn sql_create_table_merkle_stub() -> &'static str {
        r#"CREATE TABLE IF NOT EXISTS block_stub (
                source_id INTEGER NOT NULL,
                "index" INTEGER NOT NULL,
                hash BLOB
            );
        CREATE UNIQUE INDEX IF NOT EXISTS source_index ON block_stub (
            source_id,
            "index"
        );
        CREATE INDEX IF NOT EXISTS source_id ON block_stub (
            source_id
        );"#
    }

    #[cfg(feature = "mysql")]
    fn sql_create_table_source_stub() -> &'static str {
        r#"CREATE TABLE IF NOT EXISTS `leaf_stub` (
            `source_id` BIGINT UNSIGNED NOT NULL, 
            `index` INT UNSIGNED NOT NULL, 
            `hash` BLOB, 
            INDEX (`source_id`),
            UNIQUE INDEX `source_index` (`source_id`, `index`)
        ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;"#
    }

    #[cfg(feature = "sqlite")]
    fn sql_create_table_source_stub() -> &'static str {
        r#"CREATE TABLE IF NOT EXISTS leaf_stub (
                source_id INTEGER NOT NULL,
                "index" INTEGER NOT NULL,
                hash BLOB
            );
            CREATE UNIQUE INDEX IF NOT EXISTS source_index ON leaf_stub (
                source_id,
                "index"
            );
            CREATE INDEX IF NOT EXISTS source_id ON leaf_stub (
                source_id
            );"#
    }

    async fn parse_source_url(&self, url: &str) -> DmcResult<(Box<dyn ReadSeek>, u64)> {
        let source_url_parsers = self.0.source_url_parsers.read().await;
        let source_url = Url::parse(url)
            .map_err(|err| dmc_err!(DmcErrorCode::InvalidInput, "{} parse source url, url={}, err={}", self, url, err))?;
        let parser = source_url_parsers.get(source_url.scheme())
            .ok_or(dmc_err!(DmcErrorCode::InvalidInput, "no parser for scheme {}", source_url.scheme()))?;
        parser.parse(&source_url).await
            .map_err(|err| dmc_err!(DmcErrorCode::InvalidInput, "{} parse source url, url={}, err={}", self, url, err))
    }

    async fn prepare_leaf_stub_inner(&self, source: DataSource, options: CreateSourceOptions) -> DmcResult<()> {
        let update_at = source.update_at;
        info!("{} prepare source stub, source={:?}, update_at={}", self, source, update_at);

        let last_row: Option<SourceStub> = sqlx::query_as::<_, SourceStubRow>("SELECT * FROM source_stub WHERE source_id=? ORDER BY `index` DESC LIMIT 1")
            .bind(source.source_id as sqlx_types::U64).fetch_optional(self.sql_pool()).await
            .map_err(|err| dmc_err!(DmcErrorCode::Failed, "{} prepare source stub, source={:?}, update_at={}, err={}", self, source, update_at, err))?
            .map(|row| row.into());

        let count = if let Some(last_row) = last_row {
            if (last_row.index + 1) >= options.stub.count {
                0
            } else {
                options.stub.count - last_row.index - 1
            }
        } else {
            options.stub.count
        };

        let merkle_proc = MerkleProc::new(source.length, options.merkle.piece_size, options.merkle.pieces_per_block, true);

        for _ in 0..count {
            loop {
                let index = {
                    let mut rng = rand::thread_rng();
                    rng.gen_range(0u64..merkle_proc.leaves())
                };
            
                if sqlx::query("INSERT INTO leaf_stub (source_id, `index`) VALUES (?, ?)")
                    .bind(source.source_id as sqlx_types::U64).bind(index as sqlx_types::U32)
                    .execute(self.sql_pool()).await.map_err(|err| dmc_err!(DmcErrorCode::Failed, "{} prepare source stub, source={:?}, update_at={}, err={}", self, source, update_at, err))?
                    .rows_affected() == 0 {
                    debug!("{} prepare source stub, source={:?}, update_at={}, err={}", self, source, update_at, "duplicated");
                    continue;
                }
            }
           
        }

        let update_at = dmc_time_now();
        if sqlx::query("UPDATE source_state SET state_code=?, update_at=?, process_id=NULL WHERE source_id=? AND process_id=? AND update_at=? AND state_code=?")
            .bind(SourceState::PreparingMerkle as u8 as sqlx_types::U8).bind(update_at as sqlx_types::U64).bind(source.source_id as sqlx_types::U64)
            .bind(std::process::id() as sqlx_types::U32).bind(source.update_at as sqlx_types::U64).bind(source.state as u8 as sqlx_types::U8)
            .execute(self.sql_pool()).await.map_err(|err| dmc_err!(DmcErrorCode::Failed, "{} prepare source stub, source={}, update_at={}, err={}", self, source.source_id, source.update_at, err))?
            .rows_affected() > 0 {
            let mut source = source;
            source.update_at = update_at;
            source.state = SourceState::Ready;
            info!("{} prepare source merkle, source={:?}, update_at={}, finish", self, source.source_id, source.update_at);
            let server = self.clone();
            task::spawn(async move {
                let _ = server.prepare_source(source, options).await;
            });
        } else {
            info!("{} prepare source stub, source={:?}, update_at={}, ignored", self, source.source_id, source.update_at);
        }
        Ok(())
    }


    async fn prepare_merkle_inner(&self, source: DataSource, options: CreateSourceOptions) -> DmcResult<()> {
        let update_at = source.update_at;
        info!("{} prepare source merkle, source={:?}, update_at={}", self, source, update_at);
        let (source_reader, source_length) = self.parse_source_url(&source.source_url).await
            .map_err(|err| dmc_err!(DmcErrorCode::InvalidInput, "{} prepare source merkle, source={:?}, update_at={}, err=source {} {}", self, source.source_id, update_at, source.source_url, err))?;
          
        let stub_proc = MerkleProc::new(source_length, options.merkle.piece_size, options.merkle.pieces_per_block, true);
        let mut source_reader = stub_proc.wrap_reader(source_reader);

        let mut row_stream = sqlx::query_as::<_, MerkleBlockStubRow>("SELECT * FROM merkle_stub WHERE source_id=? ORDER BY `index`")
            .bind(source.source_id as sqlx_types::U64).fetch(self.sql_pool());

        let mut stubs = vec![];
        loop {
            if let Some(stub) = row_stream.next().await {
                let stub: MerkleBlockStub = stub.map(|row| row.into()).map_err(|err| dmc_err!(DmcErrorCode::Failed, "{} prepare source merkle, source={:?}, update_at={}, err={}", self, source, update_at, err))?;
                stubs.push(HashValue::try_from(stub.content).map_err(|err| dmc_err!(DmcErrorCode::Failed, "{} prepare source merkle, source={:?}, update_at={}, err={}", self, source, update_at, err))?);
            } else {
                break;
            }
        }

        if stubs.len() < stub_proc.blocks() {
            use std::io::SeekFrom;
            use async_std::io::prelude::*;
            source_reader.seek(SeekFrom::Start(stub_proc.block_size() as u64 * stubs.len() as u64)).await
                .map_err(|err| dmc_err!(DmcErrorCode::InvalidInput, "{} prepare source merkle, source={:?}, update_at={}, err=open file {} {}", self, source.source_id, update_at, source.source_url, err))?;
            for i in stubs.len()..stub_proc.blocks() {
                let content = stub_proc.calc_block_path::<_, MerkleStubSha256>(i, &mut source_reader).await
                    .map_err(|err| dmc_err!(DmcErrorCode::InvalidInput, "{} prepare source merkle, source={:?}, update_at={}, err=open file {} {}", self, source.source_id, update_at, source.source_url, err))?;

                
                sqlx::query("INSERT INTO merkle_stub (source_id, `index`, content) VALUES (?, ?, ?)")
                    .bind(source.source_id as sqlx_types::U64).bind(i as sqlx_types::U32).bind(content.as_slice())
                    .execute(self.sql_pool()).await.map_err(|err| dmc_err!(DmcErrorCode::Failed, "{} prepare source merkle, source={:?}, update_at={}, err={}", self, source, update_at, err))?;
                
                stubs.push(content);
            }
        }
        
        let root = stub_proc.calc_root_from_block_path::<MerkleStubSha256>(stubs).map_err(|err| dmc_err!(DmcErrorCode::Failed, "{} prepare source merkle, source={:?}, update_at={}, err={}", self, source, update_at, err))?;
        
        let merkle_stub = DmcMerkleStub {
            leaves: stub_proc.leaves(), 
            piece_size: stub_proc.piece_size(), 
            root
        };

        info!("{} prepare source merkle, source={:?}, update_at={}, calc merkle stub={:?}", self, source.source_id, source.update_at, merkle_stub);
        let merkle_stub_blob = serde_json::to_vec(&merkle_stub).map_err(|err| dmc_err!(DmcErrorCode::Failed, "{} prepare source merkle, source={}, update_at={}, err=clac merkel {}", self, source.source_id, update_at, err))?;

        let update_at = dmc_time_now();
        if sqlx::query("UPDATE source_state SET state_code=?, update_at=?, merkle_stub=?, process_id=NULL WHERE source_id=? AND process_id=? AND update_at=? AND state_code=?")
            .bind(SourceState::PreparingMerkle as u8 as sqlx_types::U8).bind(update_at as sqlx_types::U64).bind(merkle_stub_blob)
            .bind(source.source_id as sqlx_types::U64).bind(std::process::id() as sqlx_types::U32).bind(source.update_at as sqlx_types::U64)
            .bind(source.state as u8 as sqlx_types::U8)
            .execute(self.sql_pool()).await.map_err(|err| dmc_err!(DmcErrorCode::Failed, "{} prepare source merkle, source={}, update_at={}, err={}", self, source.source_id, source.update_at, err))?
            .rows_affected() > 0 {
            let _ = self.journal().append(JournalEvent { source_id: source.source_id, order_id: None, event_type: JournalEventType::SourcePrepared, event_params: None}).await
                .map_err(|err| dmc_err!(DmcErrorCode::Failed, "{} prepare source stub, source={:?}, update_at={}, err={}", self, source, update_at, err))?;
        } else {
            info!("{} prepare source merkle, source={:?}, update_at={}, ignored", self, source.source_id, source.update_at);
        }
        Ok(())
    }


    async fn prepare_inner(&self, source: DataSource, options: CreateSourceOptions) -> DmcResult<()> {
        let update_at = source.update_at;
        info!("{} prepare source, source={:?}, update_at={}", self, source, update_at);
        match source.state.clone() {
            SourceState::PreparingLeafStub => {
                self.prepare_leaf_stub_inner(source, options).await
            },
            SourceState::PreparingMerkle => {
                self.prepare_merkle_inner(source, options).await
            },
            SourceState::Ready => {
                info!("{} prepare source, source={:?}, update_at={}, ignored", self, source, update_at);
                Ok(())
            }
        }
    }

    #[async_recursion::async_recursion]
    async fn prepare_source(&self, source: DataSource, options: CreateSourceOptions) -> DmcResult<()> {
        let update_at = dmc_time_now();
        info!("{} prepare source, source={:?}, update_at={}", self, source, update_at);
        if sqlx::query("UPDATE source_state SET process_id=?, update_at=? WHERE source_id=? AND state_code=? AND (process_id IS NULL OR process_id!=?)")
                .bind(std::process::id() as sqlx_types::U32).bind(update_at as sqlx_types::U64).bind(source.source_id as sqlx_types::U64)
                .bind(source.state as u8 as sqlx_types::U8).bind(std::process::id() as sqlx_types::U32)
                .execute(self.sql_pool()).await.map_err(|err| dmc_err!(DmcErrorCode::Failed, "{} prepare source, source={}, update_at={}, err={}", self, source.source_id, update_at, err))?
                .rows_affected() == 0 {
            info!("{} prepare source, contract={:?}, update_at={}, ignored", self, source.source_id, update_at);
            return Ok(());
        } 

        let mut source = source;
        source.update_at = update_at;

        if self.prepare_inner(source.clone(), options).await.is_err() {
            let _ = sqlx::query("UPDATE source_state SET process_id=NULL, update_at=? WHERE source_id=? AND state_code=? AND update_at=? AND process_id=?")
                .bind(dmc_time_now() as sqlx_types::U64).bind(source.source_id as sqlx_types::U64).bind(source.state as u8 as sqlx_types::U8)
                .bind(source.update_at as sqlx_types::U64).bind(std::process::id() as sqlx_types::U32)
                .execute(self.sql_pool()).await.map_err(|err| dmc_err!(DmcErrorCode::Failed, "{} prepare source, source={}, update_at={}, err={}", self, source.source_id, update_at, err))?;
        }
        Ok(())
    }

    
}

impl SourceServer {
    pub async fn create(&self, options: CreateSourceOptions) -> DmcResult<DataSource> {
        let update_at = dmc_time_now();
        info!("{} create source, options={:?}, update_at={}", self, options, update_at);
        
        let mut conn = self.sql_pool().acquire().await
            .map_err(|err| dmc_err!(DmcErrorCode::Failed, "{} create source, options={:?}, update_at={}, err={}", self, options, update_at, err))?;

        let row = SourceStateRow::from(&options);
        let result = sqlx::query("INSERT INTO source_state (source_url, length, state_code, merkle_stub, merkle_options, stub_options, update_at) VALUES (?, ?, ?, ?, ?, ?, ?)")
            .bind(&row.source_url).bind(row.length).bind(row.state_code).bind(row.merkle_stub).bind(row.merkle_options).bind(row.stub_options).bind(update_at as sqlx_types::U64)
            .execute(&mut conn).await.map_err(|err| dmc_err!(DmcErrorCode::Failed, "{} create source, options={:?}, update_at={}, err={}", self, options, update_at, err))?;
        if result.rows_affected() > 0 {
            let row: SourceStateRow = sqlx::query_as(&format!("SELECT * FROM source_state WHERE {}=?", sqlx_types::rowid_name("source_id")))
                .bind(sqlx_types::last_inersert(&result))
                .fetch_one(&mut conn).await
                .map_err(|err| dmc_err!(DmcErrorCode::Failed, "{} create source, options={:?}, update_at={}, err={}", self, options, update_at, err))?;
            let _ = self.journal().append(JournalEvent { source_id: row.source_id as u64, order_id: None, event_type: JournalEventType::SourceCreated, event_params: None}).await
                .map_err(|err| dmc_err!(DmcErrorCode::Failed, "{} create source, options={:?}, update_at={}, err={}", self, options, update_at, err))?;
            let options: CreateSourceOptions = (&row).try_into().unwrap();
            let source: DataSource = row.try_into().unwrap();
            
            {
                let source = source.clone();
                let options = options.clone();
                let server = self.clone();
                task::spawn(async move {
                    let _ = server.prepare_source(source, options).await;
                });
            }
            info!("{} create source, options={:?}, update_at={}, finished, souce={:?}", self, options, update_at, source);
            Ok(source)
        } else {
            Err(dmc_err!(DmcErrorCode::AlreadyExists, "{} create source, options={:?}, update_at={}, ignored", self, options, update_at))
        }
    }

    pub async fn get(&self, filter: SourceFilter, navigator: SourceNavigator) -> DmcResult<Vec<DataSource>> {
        debug!("{} get source, filter={:?}, navigator={:?}", self, filter, navigator);
        if navigator.page_size == 0 {
            debug!("{} get source, filter={:?}, navigator={:?}, returns, results =0", self, filter, navigator);
            return Ok(vec![]);
        }
        
        if let Some(source_id) = filter.source_id {
            let row: Option<SourceStateRow> = sqlx::query_as("SELECT * FROM source_state WHERE source_id=?")
                .bind(source_id as sqlx_types::U64).fetch_optional(self.sql_pool()).await.map_err(|err| dmc_err!(DmcErrorCode::Failed, "{} get source, filter={:?}, navigator={:?}, err={}", self, filter, navigator, err))?;
            let sources = if let Some(row) = row {
                vec![row.try_into().map_err(|err| dmc_err!(DmcErrorCode::Failed, "{} get source, filter={:?}, navigator={:?}, err={}", self, filter, navigator, err))?]
            } else {
                vec![]
            };
            debug!("{} get source returned, filter={:?}, navigator={:?}, results={:?}", self, filter, navigator, sources);
            Ok(sources)
        } else if let Some(source_url) = &filter.source_url {
            let row: Option<SourceStateRow> = sqlx::query_as("SELECT * FROM source_state WHERE source_url=?")
                .bind(source_url).fetch_optional(self.sql_pool()).await.map_err(|err| dmc_err!(DmcErrorCode::Failed, "{} get source, filter={:?}, navigator={:?}, err={}", self, filter, navigator, err))?;
            let sources = if let Some(row) = row {
                vec![row.try_into().map_err(|err| dmc_err!(DmcErrorCode::Failed, "{} get source, filter={:?}, navigator={:?}, err={}", self, filter, navigator, err))?]
            } else {
                vec![]
            };
            debug!("{} get source returned, filter={:?}, navigator={:?}, results={:?}", self, filter, navigator, sources);
            Ok(sources)
        } else {
            unimplemented!()
        }
    }

    pub async fn random_stub(&self, source_id: u64) -> DmcResult<SourceStub> {
        // let row: SourceStateRow = sqlx::query_as("SELECT * FROM source_state WHERE source_id=?")
        //     .bind(source_id as sqlx_types::U64).fetch_one(self.sql_pool()).await.map_err(|err| dmc_err!(DmcErrorCode::Failed, "{} random stub, source={}, err={}", self, source_id, err))?;
        // let options: CreateSourceOptions = (&row).try_into().unwrap();
        // let source: DataSource = row.try_into().unwrap();

        // if source.state != SourceState::Ready {
        //     return Err(DmcError::new(DmcErrorCode::ErrorState, "not ready"));
        // }
        
        // let index = {
        //     let mut rng = rand::thread_rng();
        //     rng.gen_range(0..options.stub.count)
        // };

        // let stub = sqlx::query_as::<_, SourceStubRow>("SELECT * FROM source_stub WHERE source_id=? AND `index`=?")
        //     .bind(source_id as sqlx_types::U64).bind(index as sqlx_types::U32).fetch_one(self.sql_pool()).await
        //     .map(|row| row.into())
        //     .map_err(|err| dmc_err!(DmcErrorCode::Failed, "{} random stub, source={}, err={}", self, source_id, err))?;
        // Ok(stub)
        todo!()
    }

    // pub async fn get_source_detail(&self, source_id: u64) -> DmcResult<SourceDetailState> {
    //     let row: SourceStateRow = sqlx::query_as("SELECT * FROM source_state WHERE source_id=?")
    //         .bind(source_id as sqlx_types::U64)
    //         .fetch_one(self.sql_pool()).await
    //         .map_err(|err| dmc_err!(DmcErrorCode::Failed, "get source detail, id={}, err={}", source_id, err))?;
    //     let options: CreateSourceOptions = (&row).try_into().unwrap();
    //     let source: DataSource = row.try_into().unwrap();

    //     let (index, count) = {
    //         match source.state {
    //             SourceState::PreparingMerkle => {
    //                 let index: i64 = if let Ok(row) = sqlx::query("SELECT `index` FROM merkle_stub WHERE source_id=? ORDER BY `index` DESC LIMIT 1")
    //                     .bind(source_id as sqlx_types::U64)
    //                     .fetch_one(self.sql_pool()).await {
    //                     row.try_get(0)?
    //                 } else {
    //                     -1
    //                 };
    //                 let stub_proc = MerkleProc::new(source.length, options.merkle.piece_size(), options.merkle.pieces_per_block, true);
    //                 (index+1, stub_proc.blocks() as u64)
    //             }
    //             SourceState::PreparingStub => {
    //                 let index: i64 = if let Ok(row) = sqlx::query("SELECT `index` FROM source_stub WHERE source_id=? ORDER BY `index` DESC LIMIT 1")
    //                     .bind(source_id as sqlx_types::U64)
    //                     .fetch_one(self.sql_pool()).await {
    //                     row.try_get(0)?
    //                 } else {
    //                     -1
    //                 };
    //                 (index+1, options.stub.count as u64)
    //             }
    //             SourceState::Ready => (0, 0)
    //         }
    //     };

    //     Ok(SourceDetailState {
    //         source_id,
    //         state: source.state,
    //         index: index as u64,
    //         count,
    //     })
    // }
}