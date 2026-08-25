use agent_client_protocol::{ByteStreams, ConnectTo, Error, Role};
use tokio::net::TcpListener;
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

#[derive(Debug)]
pub struct TcpTransport {
    listener: TcpListener,
}

impl TcpTransport {
    pub async fn bind(addr: impl tokio::net::ToSocketAddrs) -> std::io::Result<Self> {
        let listener = TcpListener::bind(addr).await?;
        Ok(Self { listener })
    }
}

impl<Counterpart: Role> ConnectTo<Counterpart> for TcpTransport {
    async fn connect_to(
        self,
        client: impl ConnectTo<Counterpart::Counterpart>,
    ) -> Result<(), Error> {
        let (stream, _) = self.listener.accept().await.map_err(Error::into_internal_error)?;
        stream.set_nodelay(true).map_err(Error::into_internal_error)?;
        let (read_half, write_half) = stream.into_split();
        let component = ByteStreams::new(write_half.compat_write(), read_half.compat());
        ConnectTo::<Counterpart>::connect_to(component, client).await?;
        Ok(())
    }
}
