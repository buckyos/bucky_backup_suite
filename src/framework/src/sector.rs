use std::{io, sync::mpsc::RecvTimeoutError};


struct CheckPoint {
    pub uuid: Uuid,
    pub rely_on: Uuid,
    pub timestamp: Timestamp,
    pub actions: Vec<Action>,
}

const MAX_SECTOR_LENGTH: u64 = 32 * 1024 * 1024 * 1024;
const SECTOR_MAGIC: u32 = 0x12345678;
const SECTOR_VERSION_0: u16 = 0;

const SECTOR_FLAG_ENCRYPT: u32 = 0x00000001;
const SECTOR_FLAG_SIGN: u32 = 0x00000002;


// restore meta from stream
fn restore_meta_from_stream<R: io::Read + io::Seek>(
    stream:R, 
    sk: SecretKey, 
    checkpoints: &mut Vec<CheckPoint>, 
    last_checkpoint_offset: u64,
) -> u64 {

    let mut md = MessageDigest::sha256();
    let mut buffer = [0u8; 32];

    // ignore some header parts, magic, version, flags


    // read public key
    stream.read_exact(&mut buffer).unwrap();
    let pk = PublicKey::from_bytes(&buffer).unwrap();
    md.update(&buffer);

    // verify public key is mine
    assert_eq!(pk, PublicKey::from_secret(&sk));

    // read encrypted key
    stream.read_exact(&mut buffer).unwrap();
    md.update(&buffer);
    let encrypted_key = EncryptedKey::from_bytes(&buffer).unwrap();
    
    // decrypt aes key
    let aes_key = sk.decrypt(&encrypted_key).unwrap();

    // read signature
    stream.read_exact(&mut buffer).unwrap();
    md.update(&buffer);
    let signature = Signature::from_bytes(&buffer).unwrap();

    // decrypt stream with aes key 
    let mut box_stream = DecryptStream::new(&mut stream, &aes_key);
    
    // verify signature
    // read uuid
    box_stream.read_exact(&mut buffer).unwrap();
    let uuid = Uuid::from_bytes(&buffer).unwrap();

    // read offset in checkpoint
    box_stream.read_exact(&mut buffer[0..8]).unwrap();
    let offset_in_checkpoint = u64::from_be_bytes(&buffer);

    if let Some(last_checkpoint) = checkpoints.last_mut() {
        if last_checkpoint.uuid == uuid {
            // this is a continuation of last checkpoint
            assert_eq!(offset_in_checkpoint, last_checkpoint_offset);
        } else {
            // that's new checkpoint
            assert_eq!(offset_in_checkpoint, 0);
        }
    }

    // read box length
    box_stream.read_exact(&mut buffer[0..8]).unwrap();
    let box_length = u64::from_be_bytes(&buffer);
    
    
}


fn backup_to_sector(checkpoint: Checkpoint) {
   

    for action in checkpoint.actions {

    } 


    loop {
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

        // placeholder for signature
        io::skip(sector_tmp, 32);
        
        // split last check point part into this sector
        if let Some(cur_context) = cur_context {
            
        }
       
    }

}