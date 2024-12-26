mod provider;
mod local_chunk_provider;
pub use provider::*;
pub use local_chunk_provider::*;


pub struct DiffObject {
    pub diff_type:String,
    pub diff_data:Vec<u8>,
}
