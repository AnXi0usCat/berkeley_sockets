use std::sync::Arc;
use std::task::Poll;

use crate::{reactor::Reactor, socket::Socket};

pub struct AcceptFuture<'a> {
    listener: &'a Socket,
    reactor: Arc<Reactor>,
}

impl<'a> Future for AcceptFuture<'a> {
    type Output = Result<AsyncTcpStream, String>;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        match self.listener.accept_nonblocking() {
            Ok(Some(socket)) => Poll::Ready(Ok(AsyncTcpStream::new(socket, self.reactor.clone()))),
            Ok(None) => {
                self.reactor
                    .register(self.listener.fd, true, false, cx.waker())?;
                Poll::Pending
            }
            Err(e) => Poll::Ready(Err(e)),
        }
    }
}

#[derive(Debug)]
pub struct AsyncTcpListener {
    socket: Socket,
    reactor: Arc<Reactor>,
}

impl AsyncTcpListener {
    pub fn bind(host: &str, port: u16) -> Result<Self, String> {
        let mut sc = Socket::new()?;
        sc.bind(host, port)?;
        sc.set_nonblocking(true)?;
        sc.listen(100)?;

        let reactor = Reactor::new()?;

        Ok(AsyncTcpListener {
            socket: sc,
            reactor: Arc::new(reactor),
        })
    }

    pub fn accept(&self) -> AcceptFuture {
        AcceptFuture {
            listener: &self.socket,
            reactor: self.reactor.clone(),
        }
    }
}

#[derive(Debug)]
pub struct AsyncTcpStream {
    socket: Socket,
    reactor: Arc<Reactor>,
    read_buf: Vec<u8>,
    write_buf: Vec<u8>
}

impl AsyncTcpStream {
    pub fn new(socket: Socket, reactor: Arc<Reactor>) -> Self {
        AsyncTcpStream { 
            socket,
            reactor,
            read_buf: vec![0; 1024],
            write_buf: Vec::new()
        }
    }
}
