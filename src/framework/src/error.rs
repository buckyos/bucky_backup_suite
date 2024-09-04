#[derive(Debug, Error)]
pub enum BackupError {
    #[error("Ok")]
    Ok,
    #[error("Failed for: {0}")]
    Failed(String),
    #[error("InvalidArgument: {0}")]
    InvalidArgument(String),
    #[error("NotFound: {0}")]
    NotFound(String),
    #[error("AlreadyExists: {0}")]
    AlreadyExists(String),
    #[error("ErrorState: {0}")]
    ErrorState(String),
}

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

#[macro_export]
macro_rules! handle_error {
    ($fmt:expr $(, $($arg:tt)*)?) => {{
        move |err| {
            let err_msg = match format_args!($fmt $(, $($arg)*)?) {
                args => format!("{}: {}", args, err),
            };
            log::error!("{}", err_msg);
            err
        }
    }};
}
