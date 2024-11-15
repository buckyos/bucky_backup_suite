use std::{
    pin::Pin,
    task::{Poll, Context}
};
use async_std::io::prelude::*;
use rs_merkle::*;
use crate::{
    error::*,
    utils::*, 
};

// peddging to full tree
pub struct MerkleStubPedding {

}

pub struct MerkleStubPeddingReader<R: async_std::io::Read + async_std::io::Seek + Unpin> {
    pedding_length: u64, 
    source_length: u64, 
    source_reader: R,
    offset: u64
}

impl<R: async_std::io::Read + async_std::io::Seek + Unpin> MerkleStubPeddingReader<R> {
    fn new(pedding_length: u64, source_length: u64, source_reader: R) -> Self {
        Self {
            pedding_length,
            source_length,
            source_reader, 
            offset: 0
        }
    }
}


impl<R: async_std::io::Read + async_std::io::Seek + Unpin> async_std::io::Read for MerkleStubPeddingReader<R> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<std::io::Result<usize>> {
        let reader = self.get_mut();
        if reader.offset >= reader.pedding_length {
            return Poll::Ready(Ok(0));
        }

        if reader.offset >= reader.source_length {
            let read = u64::min(reader.pedding_length - reader.offset, buf.len() as u64) as usize;
            buf[0..read].fill(0u8);
            reader.offset += read as u64;
            return Poll::Ready(Ok(read));
        }

        Pin::new(&mut reader.source_reader).poll_read(cx, buf).map(|result| {
            result.map(|read| {
                reader.offset += read as u64;
                read
            })
        }) 
    }
}

impl<R: async_std::io::Read + async_std::io::Seek + Unpin> async_std::io::Seek for MerkleStubPeddingReader<R> {
    fn poll_seek(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        pos: std::io::SeekFrom,
    ) -> Poll<std::io::Result<u64>> {
        let reader = self.get_mut();
        use std::io::SeekFrom::*;
        match pos {
            Start(offset) => {
                let new_offset = u64::min(offset, reader.pedding_length);
                if new_offset >= reader.source_length {
                    reader.offset = new_offset;
                    return Poll::Ready(Ok(new_offset));
                }
        
                Pin::new(&mut reader.source_reader).poll_seek(cx, Start(offset)).map(|result| {
                    result.map(|new_offset| {
                        reader.offset = new_offset;
                        new_offset
                    })
                }) 
            },
            _ => unreachable!()
        }
    }
}


pub struct MerkleProc {
    source_length: u64,
    piece_size: u16,  
    leaves: u64, 
    pieces_per_block: u16, 
    blocks: usize, 
}


// #[derive(Clone)]
// pub struct MerkleStubChallenge<H: Hasher> {
//     pub piece_index: u64, 
//     pub stub: H::Hash, 
//     pub pathes: Vec<H::Hash>
// }

// #[derive(Clone)]
// pub struct MerkleStubProof<H: Hasher> {
//     pub piece_index: u64, 
//     pub piece_content: Vec<u8>,
//     pub pathes: Vec<H::Hash>
// }


impl MerkleProc {
    pub fn verifier(leaves: u64, piece_size: u16, pieces_per_block: u16) -> Self {
        let blocks = (leaves / pieces_per_block as u64) as usize;

        Self {
            source_length: 0, 
            piece_size, 
            leaves, 
            pieces_per_block, 
            blocks, 
        }
    }
    
    pub fn new(source_length: u64, piece_size: u16, pieces_per_block: u16, pedding: bool) -> Self {
        let leaves = if source_length % piece_size as u64 == 0 {
            source_length / piece_size as u64
        } else {
            source_length / piece_size as u64 + 1
        };


        let leaves = if pedding {
            let degree = u64::BITS - leaves.leading_zeros() - 1;
            if leaves == 1u64 << degree {
                leaves
            } else {
                1u64 << (degree + 1)
            }
        } else {
            leaves
        };

        let blocks = (leaves / pieces_per_block as u64) as usize;

        Self {
            source_length, 
            piece_size, 
            leaves, 
            pieces_per_block, 
            blocks, 
        }
    }

    pub fn pedding_length(&self) -> u64 {
        self.leaves * self.piece_size as u64
    }

    pub fn wrap_reader<R: async_std::io::Read + async_std::io::Seek + Unpin>(&self, source_reader: R) -> MerkleStubPeddingReader<R> {
        MerkleStubPeddingReader::new(self.pedding_length(), self.source_length, source_reader)
    }

    pub fn leaves(&self) -> u64 {
        self.leaves
    }

    pub fn piece_size(&self) -> u16 {
        self.piece_size
    }

    pub fn pieces_per_block(&self) -> usize {
        self.pieces_per_block as usize
    }

    pub fn blocks(&self) -> usize {
        self.blocks
    }

    pub fn block_size(&self) -> usize {
        self.piece_size as usize * self.pieces_per_block as usize
    }

  
    async fn block_path_tree<R: async_std::io::Read + async_std::io::Seek + Unpin, H: Hasher>(&self, block_index: usize, reader: &mut R) -> DmcResult<MerkleTree<H>> {
        let mut piece = vec![0u8; self.piece_size as usize];
        let mut leaves = vec![];

        let mut source_offset = block_index as u64 * self.block_size() as u64;
        assert!(source_offset < self.pedding_length());
        use async_std::io::SeekFrom;

        reader.seek(SeekFrom::Start(source_offset)).await?;
        loop {
            let to_read = u64::min(self.piece_size as u64, self.pedding_length() - source_offset) as usize; 
            if to_read > 0 {
                let _ = reader.read_exact(&mut piece[0..to_read]).await;
                let leaf = H::hash(&piece[0..to_read]);
                leaves.push(leaf);

                if leaves.len() >= self.pieces_per_block() {
                    break;
                }
            }
            if to_read < self.piece_size() as usize {
                break;
            }
            source_offset += to_read as u64;
        }

        let mut merkle_tree = MerkleTree::<H>::new();
        merkle_tree.append(&mut leaves);

        Ok(merkle_tree)
    }


    pub async fn calc_block_path<R: async_std::io::Read + async_std::io::Seek + Unpin, H: Hasher>(&self, block_index: usize, reader: &mut R) -> DmcResult<H::Hash> {
        let mut merkle_tree = self.block_path_tree::<_, H>(block_index, reader).await?;
        merkle_tree.commit();
        Ok(merkle_tree.root().unwrap())
    } 

    pub fn calc_root_from_block_path<H: Hasher>(&self, block_roots: Vec<H:: Hash>) -> DmcResult<H::Hash> {
        let mut block_roots = block_roots;
        let mut merkle_tree = MerkleTree::<H>::new();
        merkle_tree.append(&mut block_roots).commit();
        Ok(merkle_tree.root().unwrap())
    }

    pub async fn calc_root_from_pieces<R: async_std::io::Read + Unpin, H: Hasher>(&self, reader: &mut R) -> DmcResult<H::Hash> {
        let mut piece = vec![0u8; self.piece_size as usize];
        let mut leaves = vec![];

        let mut read = 0;
        loop {
            let to_read = u64::min(self.piece_size as u64, self.pedding_length() - read) as usize; 
            let _ = reader.read_exact(&mut piece[0..to_read]).await;
            let leaf = H::hash(&piece[0..to_read]);
            leaves.push(leaf);
            read += to_read as u64;
            if read >= self.pedding_length() {
                break;
            }
        }
        
        let mut merkle_tree = MerkleTree::<H>::new();
        merkle_tree.append(&mut leaves).commit();
        Ok(merkle_tree.root().unwrap())
    }

    pub fn block_index_of_piece(&self, piece_index: u64) -> usize {
        assert!(piece_index < self.leaves);
        (piece_index / self.pieces_per_block as u64) as usize
    }
}

//     pub fn challenge_of_piece<H: Hasher>(&self, piece_index: u64, block_roots: Vec<H::Hash>) -> MerkleStubChallenge<H> {
//         let block_index = self.block_index_of_piece(piece_index);
//         let block_ = block_roots[block_index].clone();
//         let mut block_roots = block_roots;
//         let mut merkle_tree = MerkleTree::<H>::new();
//         merkle_tree.append(&mut block_roots);
//         merkle_tree.commit();

//         MerkleStubChallenge {
//             piece_index, 
//             stub, 
//             pathes: Vec::from(merkle_tree.proof(&[stub_index]).proof_hashes())
//         }  
//     }

//     pub fn verify_stub_challenge<H: Hasher>(&self, root: H::Hash, challenge: MerkleStubChallenge<H>) -> DmcResult<bool> {
//         let proof = MerkleProof::<H>::new(challenge.pathes);
//         let stub_index = self.stub_index_of_piece(challenge.piece_index);
//         Ok(proof.verify(
//             root, 
//             &[stub_index], 
//             &[challenge.stub], 
//             self.stub_count()))
//     }

//     pub async fn proof_of_piece<R: async_std::io::Read + async_std::io::Seek + Unpin, H: Hasher>(&self, piece_index: u64, reader: &mut R) -> DmcResult<MerkleStubProof<H>> {
//         let stub_index = self.stub_index_of_piece(piece_index);
//         let mut merkle_tree = self.path_stub_tree::<_, H>(stub_index, reader).await?;
//         merkle_tree.commit();

//         let source_offset = self.piece_size() as u64 * piece_index as u64;
//         use async_std::io::SeekFrom;
//         let _ = reader.seek(SeekFrom::Start(source_offset)).await?;

//         let piece_size = u64::min(self.piece_size as u64, self.pedding_length() - source_offset) as usize;
//         let mut piece_content = vec![0u8; piece_size];
//         let _ = reader.read_exact(piece_content.as_mut()).await;


//         let piece_index_in_stub = (piece_index - stub_index as u64 * self.stub_piece_count() as u64) as usize;

//         Ok(MerkleStubProof {
//             piece_index, 
//             piece_content, 
//             pathes: Vec::from(merkle_tree.proof(&[piece_index_in_stub]).proof_hashes())
//         })
//     }

//     pub fn verify_stub_proof<H: Hasher>(&self, stub: H::Hash, proof: MerkleStubProof<H>) -> DmcResult<bool> {
//         let tree_proof = MerkleProof::<H>::new(proof.pathes);
//         let piece_index_stub = (proof.piece_index % self.stub_piece_count() as u64) as usize; 
//         let leaf = H::hash(proof.piece_content.as_slice());

//         if tree_proof.proof_hashes().len() == 0 {
//             Ok(leaf == stub)
//         } else {
//             Ok(tree_proof.verify(stub, &[piece_index_stub], &[leaf], self.stub_piece_count()))
//         }
//     }

//     pub async fn proof_root<R, H>(&self, piece_index: u64, reader: &mut R, stubs: Option<Vec<H::Hash>>, piece_content: Option<Vec<u8>>) -> DmcResult<MerkleStubProof<H>>
//         where R: async_std::io::Read + async_std::io::Seek + Unpin, H: Hasher {
//         if let Some(stubs) = stubs {
//             let upper_part = self.challenge_of_piece::<H>(piece_index, stubs);

//             let lower_part = self.proof_of_piece::<_, H>(piece_index, reader).await?;
                    
//             let pathes = vec![lower_part.pathes, /*vec![upper_part.stub],*/ upper_part.pathes].concat();

//             Ok(MerkleStubProof {
//                 piece_index, 
//                 piece_content: lower_part.piece_content,
//                 pathes: pathes
//             })
//         } else if let Some(piece_content) = piece_content {
//             let mut piece = vec![0u8; self.piece_size as usize];
//             let mut leaves = vec![];
    
//             let mut read = 0;
//             loop {
//                 let to_read = u64::min(self.piece_size as u64, self.pedding_length() - read) as usize; 
//                 let _ = reader.read_exact(&mut piece[0..to_read]).await;
//                 let leaf = H::hash(&piece[0..to_read]);
//                 leaves.push(leaf);
//                 read += to_read as u64;
//                 if read >= self.pedding_length() {
//                     break;
//                 }
//             }
            
//             let mut merkle_tree = MerkleTree::<H>::new();
//             merkle_tree.append(&mut leaves).commit();
//             let pathes = Vec::from(merkle_tree.proof(&[piece_index as usize]).proof_hashes());

//             Ok(MerkleStubProof {
//                 piece_index, 
//                 piece_content,
//                 pathes
//             })
//         } else {
//             unimplemented!()
//         }
//     }

//     pub fn verify_root<H: Hasher>(&self, root: H::Hash, piece_index: u64, proof: MerkleStubProof<H>) -> DmcResult<bool> {
//         let leaf = H::hash(proof.piece_content.as_slice());
//         let tree_proof = MerkleProof::<H>::new(proof.pathes);

//         Ok(tree_proof.verify(root, &[piece_index as usize], &[leaf], self.leaves as usize))
//     }
// }

use sha2::{digest::FixedOutput, Digest, Sha256};
#[derive(Clone)]
pub struct MerkleStubSha256 {}

impl Hasher for MerkleStubSha256 {
    type Hash = HashValue;

    fn hash(data: &[u8]) -> HashValue {
        let mut hasher = Sha256::new();

        hasher.update(data);
        HashValue::from(hasher.finalize_fixed())
    }
}

// #[async_std::test]
// async fn test_merkle_path_stub() {
//     fn random_mem(piece: usize, count: usize) -> (usize, Vec<u8>) {
//         let mut buffer = vec![0u8; piece * count];
//         for i in 0..count {
//             let piece_ptr = &mut buffer[i * piece..(i + 1) * piece];
//             let bytes = count.to_be_bytes();
//             for j in 0..(piece >> 3) {
//                 piece_ptr[j * 8..(j + 1) * 8].copy_from_slice(&bytes[..]);
//             }
//         }
//         (piece * count, buffer)
//     }

//     {
//         let piece_count = 1024;
//         let (len, buffer) = random_mem(1024, piece_count);
//         use async_std::io::Cursor;

//         let proc1 = MerkleStubProc::new(len as u64, 1024, false);

//         assert_eq!(proc1.total_degree(), 10);
//         assert_eq!(proc1.stub_degree(), 5);
//         assert_eq!(proc1.stub_count(), 32);
//         assert_eq!(proc1.stub_piece_count(), 32); 

//         let mut stubs = vec![];
//         for i in 0..proc1.stub_count() {
//             stubs.push(proc1.calc_path_stub::<_, MerkleStubSha256>(i, &mut proc1.wrap_reader(Cursor::new(buffer.as_slice()))).await.unwrap());
//         }
//         let root1 = proc1.calc_root_from_path_stub::<MerkleStubSha256>(stubs.clone()).unwrap();


//         let proc2 = MerkleStubProc::new(len as u64, 1024, false);
//         let root2 = proc2.calc_root::<_, MerkleStubSha256>(&mut proc2.wrap_reader(Cursor::new(buffer.as_slice()))).await.unwrap();
        
//         assert_eq!(root1, root2);

//         {
//             let piece_index = 123;
//             assert_eq!(proc2.stub_index_of_piece(piece_index), 3);
//             let challenge = proc2.challenge_of_piece::<MerkleStubSha256>(piece_index, stubs.clone());
//             assert_eq!(challenge.piece_index, piece_index);
//             // assert_eq!(&challenge.stub, &stubs[proc2.stub_index_of_piece(piece_index)]);
//             assert_eq!(challenge.pathes.len(), 5);
//             assert!(proc2.verify_stub_challenge::<MerkleStubSha256>(root1.clone(), challenge.clone()).unwrap());

//             let proof = proc2.proof_of_piece::<_, MerkleStubSha256>(piece_index, &mut proc2.wrap_reader(Cursor::new(buffer.as_slice()))).await.unwrap();
//             assert_eq!(proof.pathes.len(), 5);
//             assert_eq!(proof.piece_index, piece_index);
//             assert!(proc2.verify_stub_proof::<MerkleStubSha256>(HashValue::try_from(challenge.stub.clone()).unwrap(), proof.clone()).unwrap());

//             let proof1 = proc2.proof_root::<_, MerkleStubSha256>(piece_index, &mut proc2.wrap_reader(Cursor::new(buffer.as_slice())), None, Some(vec![])).await.unwrap();
        
//             let proof2 = proc2.proof_root::<_, MerkleStubSha256>(piece_index, &mut proc2.wrap_reader(Cursor::new(buffer.as_slice())), Some(stubs.clone()), None).await.unwrap();

//             assert_eq!(proof1.pathes.len(), proof2.pathes.len());

//             assert!(proc2.verify_root(root2.clone(), piece_index, proof2).unwrap());
//         }


//         {
//             let piece_index = 555;
//             let challenge = proc2.challenge_of_piece::<MerkleStubSha256>(piece_index, stubs.clone());
//             assert!(proc2.verify_stub_challenge::<MerkleStubSha256>(root1.clone(), challenge.clone()).unwrap());

//             let proof = proc2.proof_of_piece::<_, MerkleStubSha256>(piece_index, &mut proc2.wrap_reader(Cursor::new(buffer.as_slice()))).await.unwrap();
//             assert!(proc2.verify_stub_proof::<MerkleStubSha256>(HashValue::try_from(challenge.stub).unwrap(), proof).unwrap())
//         }

//     }

//     {
//         let piece_count = 1025;
//         let (len, buffer) = random_mem(1024, piece_count);
//         use async_std::io::Cursor;

//         let proc1 = MerkleStubProc::new(len as u64, 1024, false);

//         assert_eq!(proc1.total_degree(), 11);
//         assert_eq!(proc1.stub_degree(), 5);
//         assert_eq!(proc1.stub_count(), 17);
//         assert_eq!(proc1.stub_piece_count(), 64); 

//         let mut stubs = vec![];
//         for i in 0..proc1.stub_count() {
//             stubs.push(proc1.calc_path_stub::<_, MerkleStubSha256>(i, &mut proc1.wrap_reader(Cursor::new(buffer.as_slice()))).await.unwrap());
//         }
//         let root1 = proc1.calc_root_from_path_stub::<MerkleStubSha256>(stubs.clone()).unwrap();


//         let proc2 = MerkleStubProc::new(len as u64, 1024, false);
//         let root2 = proc2.calc_root::<_, MerkleStubSha256>(&mut proc2.wrap_reader(Cursor::new(buffer.as_slice()))).await.unwrap();
        
//         assert_eq!(root1, root2);

//         {
//             let piece_index = 123;
//             assert_eq!(proc2.stub_index_of_piece(piece_index), 1);
//             let challenge = proc2.challenge_of_piece::<MerkleStubSha256>(piece_index, stubs.clone());
//             assert_eq!(challenge.piece_index, piece_index);
//             // assert_eq!(&challenge.stub, &stubs[proc2.stub_index_of_piece(piece_index)]);
//             assert_eq!(challenge.pathes.len(), 5);
//             assert!(proc2.verify_stub_challenge::<MerkleStubSha256>(root1.clone(), challenge.clone()).unwrap());

//             let proof = proc2.proof_of_piece::<_, MerkleStubSha256>(piece_index, &mut proc2.wrap_reader(Cursor::new(buffer.as_slice()))).await.unwrap();
//             assert_eq!(proof.pathes.len(), 6);
//             assert_eq!(proof.piece_index, piece_index);
//             assert!(proc2.verify_stub_proof::<MerkleStubSha256>(HashValue::try_from(challenge.stub).unwrap(), proof).unwrap())
//         }


//         {
//             let piece_index = 1024;
//             let challenge = proc2.challenge_of_piece::<MerkleStubSha256>(piece_index, stubs.clone());
//             assert_eq!(challenge.piece_index, piece_index);
//             // assert_eq!(&challenge.stub, &stubs[proc2.stub_index_of_piece(piece_index)]);
//             assert_eq!(challenge.pathes.len(), 1);
//             assert!(proc2.verify_stub_challenge::<MerkleStubSha256>(root1.clone(), challenge.clone()).unwrap());

//             let proof = proc2.proof_of_piece::<_, MerkleStubSha256>(piece_index, &mut proc2.wrap_reader(Cursor::new(buffer.as_slice()))).await.unwrap();
//             assert_eq!(proof.pathes.len(), 0);
//             assert_eq!(proof.piece_index, piece_index);
//             assert!(proc2.verify_stub_proof::<MerkleStubSha256>(HashValue::try_from(challenge.stub).unwrap(), proof).unwrap())
//         }

//     }


//     // {
//     //     let piece_count = 1024 * 333;
//     //     let (len, buffer) = random_mem(1024, piece_count);
//     //     use async_std::io::Cursor;

//     //     let proc1 = MerkleStubProc::new(len as u64, 1024);

//     //     let mut stubs = vec![];
//     //     for i in 0..proc1.stub_count() {
//     //         stubs.push(proc1.calc_path_stub::<_, MerkleStubSha256>(i, &mut Cursor::new(buffer.as_slice())).await.unwrap());
//     //     }
//     //     let root1 = proc1.calc_root_from_path_stub::<MerkleStubSha256>(stubs.clone()).unwrap();


//     //     let proc2 = MerkleStubProc::new(len as u64, 1024);
//     //     let root2 = proc2.calc_root::<_, MerkleStubSha256>(&mut Cursor::new(buffer.as_slice())).await.unwrap();
        
//     //     assert_eq!(root1, root2);
//     // }
    
// }





// #[async_std::test]
// async fn test_pedding_merkle_path_stub() {
//     fn random_mem(piece: usize, count: usize) -> (usize, Vec<u8>) {
//         let mut buffer = vec![0u8; piece * count];
//         for i in 0..count {
//             let piece_ptr = &mut buffer[i * piece..(i + 1) * piece];
//             let bytes = count.to_be_bytes();
//             for j in 0..(piece >> 3) {
//                 piece_ptr[j * 8..(j + 1) * 8].copy_from_slice(&bytes[..]);
//             }
//         }
//         (piece * count, buffer)
//     }

//     {
//         let piece_count = 1024;
//         let (len, buffer) = random_mem(1024, piece_count);
//         use async_std::io::Cursor;

//         let proc1 = MerkleStubProc::new(len as u64, 1024, true);


//         let mut stubs = vec![];
//         for i in 0..proc1.stub_count() {
//             stubs.push(proc1.calc_path_stub::<_, MerkleStubSha256>(i, &mut proc1.wrap_reader(Cursor::new(buffer.as_slice()))).await.unwrap());
//         }
//         let root1 = proc1.calc_root_from_path_stub::<MerkleStubSha256>(stubs.clone()).unwrap();


//         let proc2 = MerkleStubProc::new(len as u64, 1024, true);
//         let root2 = proc2.calc_root::<_, MerkleStubSha256>(&mut proc2.wrap_reader(Cursor::new(buffer.as_slice()))).await.unwrap();
        
//         assert_eq!(root1, root2);

//         {
//             let piece_index = 123;
//             let challenge = proc2.challenge_of_piece::<MerkleStubSha256>(piece_index, stubs.clone());
//             assert_eq!(challenge.piece_index, piece_index);
//             // assert_eq!(&challenge.stub, &stubs[proc2.stub_index_of_piece(piece_index)]);
//             assert!(proc2.verify_stub_challenge::<MerkleStubSha256>(root1.clone(), challenge.clone()).unwrap());

//             let proof = proc2.proof_of_piece::<_, MerkleStubSha256>(piece_index, &mut proc2.wrap_reader(Cursor::new(buffer.as_slice()))).await.unwrap();
//             assert_eq!(proof.piece_index, piece_index);
//             assert!(proc2.verify_stub_proof::<MerkleStubSha256>(HashValue::try_from(challenge.stub.clone()).unwrap(), proof.clone()).unwrap());

//             let proof1 = proc2.proof_root::<_, MerkleStubSha256>(piece_index, &mut proc2.wrap_reader(Cursor::new(buffer.as_slice())), None, Some(vec![])).await.unwrap();
        
//             let proof2 = proc2.proof_root::<_, MerkleStubSha256>(piece_index, &mut proc2.wrap_reader(Cursor::new(buffer.as_slice())), Some(stubs.clone()), None).await.unwrap();

//             assert_eq!(proof1.pathes.len(), proof2.pathes.len());

//             assert!(proc2.verify_root(root2.clone(), piece_index, proof2).unwrap());
//         }


//         {
//             let piece_index = 555;
//             let challenge = proc2.challenge_of_piece::<MerkleStubSha256>(piece_index, stubs.clone());
//             assert!(proc2.verify_stub_challenge::<MerkleStubSha256>(root1.clone(), challenge.clone()).unwrap());

//             let proof = proc2.proof_of_piece::<_, MerkleStubSha256>(piece_index, &mut proc2.wrap_reader(Cursor::new(buffer.as_slice()))).await.unwrap();
//             assert!(proc2.verify_stub_proof::<MerkleStubSha256>(HashValue::try_from(challenge.stub).unwrap(), proof).unwrap())
//         }

//     }

//     {
//         let piece_count = 1025;
//         let (len, buffer) = random_mem(1024, piece_count);
//         use async_std::io::Cursor;

//         let proc1 = MerkleStubProc::new(len as u64, 1024, true);

//         let mut stubs = vec![];
//         for i in 0..proc1.stub_count() {
//             stubs.push(proc1.calc_path_stub::<_, MerkleStubSha256>(i, &mut proc1.wrap_reader(Cursor::new(buffer.as_slice()))).await.unwrap());
//         }
//         let root1 = proc1.calc_root_from_path_stub::<MerkleStubSha256>(stubs.clone()).unwrap();


//         let proc2 = MerkleStubProc::new(len as u64, 1024, true);
//         let root2 = proc2.calc_root::<_, MerkleStubSha256>(&mut proc2.wrap_reader(Cursor::new(buffer.as_slice()))).await.unwrap();
        
//         assert_eq!(root1, root2);

//         {
//             let piece_index = 123;
//             let challenge = proc2.challenge_of_piece::<MerkleStubSha256>(piece_index, stubs.clone());
//             assert_eq!(challenge.piece_index, piece_index);
//             // assert_eq!(&challenge.stub, &stubs[proc2.stub_index_of_piece(piece_index)]);
//             assert!(proc2.verify_stub_challenge::<MerkleStubSha256>(root1.clone(), challenge.clone()).unwrap());

//             let proof = proc2.proof_of_piece::<_, MerkleStubSha256>(piece_index, &mut proc2.wrap_reader(Cursor::new(buffer.as_slice()))).await.unwrap();
//             assert_eq!(proof.piece_index, piece_index);
//             assert!(proc2.verify_stub_proof::<MerkleStubSha256>(HashValue::try_from(challenge.stub).unwrap(), proof).unwrap())
//         }


//         {
//             let piece_index = 1024;
//             let challenge = proc2.challenge_of_piece::<MerkleStubSha256>(piece_index, stubs.clone());
//             assert_eq!(challenge.piece_index, piece_index);
//             // assert_eq!(&challenge.stub, &stubs[proc2.stub_index_of_piece(piece_index)]);
//             assert!(proc2.verify_stub_challenge::<MerkleStubSha256>(root1.clone(), challenge.clone()).unwrap());

//             let proof = proc2.proof_of_piece::<_, MerkleStubSha256>(piece_index, &mut proc2.wrap_reader(Cursor::new(buffer.as_slice()))).await.unwrap();
//             assert_eq!(proof.piece_index, piece_index);
//             assert!(proc2.verify_stub_proof::<MerkleStubSha256>(HashValue::try_from(challenge.stub).unwrap(), proof).unwrap())
//         }

//     }


//     // {
//     //     let piece_count = 1024 * 333;
//     //     let (len, buffer) = random_mem(1024, piece_count);
//     //     use async_std::io::Cursor;

//     //     let proc1 = MerkleStubProc::new(len as u64, 1024);

//     //     let mut stubs = vec![];
//     //     for i in 0..proc1.stub_count() {
//     //         stubs.push(proc1.calc_path_stub::<_, MerkleStubSha256>(i, &mut Cursor::new(buffer.as_slice())).await.unwrap());
//     //     }
//     //     let root1 = proc1.calc_root_from_path_stub::<MerkleStubSha256>(stubs.clone()).unwrap();


//     //     let proc2 = MerkleStubProc::new(len as u64, 1024);
//     //     let root2 = proc2.calc_root::<_, MerkleStubSha256>(&mut Cursor::new(buffer.as_slice())).await.unwrap();
        
//     //     assert_eq!(root1, root2);
//     // }
    
// }



