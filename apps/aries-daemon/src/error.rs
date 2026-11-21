use std::io;

use aries_ipc::ServeError;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to register {signal}: {source}")]
    Signal { signal: &'static str, source: io::Error },
    #[error(transparent)]
    Serve(ServeError),
}

impl Error {
    pub fn signal(signal: &'static str, source: io::Error) -> Self {
        Self::Signal { signal, source }
    }

    pub fn serve(source: ServeError) -> Self {
        Self::Serve(source)
    }
}
