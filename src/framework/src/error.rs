#[derive(Debug, Clone)]
pub enum BackupError {}

impl std::fmt::Display for BackupError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        // write!(f, "BackupError")
        unimplemented!()
    }
}

impl std::error::Error for BackupError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        unimplemented!()
    }

    // fn provide<'a>(&'a self, request: &mut std::error::Request<'a>) {
    //     unimplemented!()
    // }
}

pub type BackupResult<T> = std::result::Result<T, BackupError>;
