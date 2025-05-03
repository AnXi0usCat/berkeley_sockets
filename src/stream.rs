use std::sync::Arc;

use crate::reactor::{CmdTx, Event, EventRx};
use crate::{
    reactor::{Cmd, Reactor},
    socket::Socket,
};

#[derive(Debug)]
pub struct AsyncTcpListener {
    socket: Socket,
    reactor: Reactor,
    cmd_tx: CmdTx,
    event_rx: Arc<EventRx>,
}

impl AsyncTcpListener {
    pub fn bind(host: &str, port: u16) -> Result<Self, String> {
        let mut sc = Socket::new()?;
        sc.bind(host, port)?;
        sc.set_nonblocking(true)?;
        sc.listen(100)?;

        let reactor = Reactor::new(sc.fd)?;
        let (tx, rx) = reactor.start();

        tx.send(Cmd::Add(sc.fd, true, false))
            .map_err(|_| "failed to add listener to kqueue")?;

        Ok(AsyncTcpListener {
            socket: sc,
            reactor,
            cmd_tx: tx,
            event_rx: Arc::new(rx),
        })
    }

    pub async fn accept(&self) -> Result<AsyncTcpStream, String> {
        while let Ok(event) = self.event_rx.try_recv() {
            if let Event::NewConnection(_fd) = event {
                if let Ok(client) = self.socket.accept_nonblocking() {
                    match client {
                        Some(socket) => {
                            self.cmd_tx
                                .send(Cmd::Add(socket.fd, true, false))
                                .map_err(|e| {
                                    format!("Failed to send client fd to the reactor: {e}")
                                })?;
                            return Ok(AsyncTcpStream::new(
                                socket,
                                self.cmd_tx.clone(),
                                Arc::clone(&self.event_rx),
                            ));
                        }
                        None => {
                            continue;
                        }
                    }
                };
            };
        }
        Err("Stream channle is closed".into())
    }
}

#[derive(Debug)]
pub struct AsyncTcpStream {
    socket: Socket,
    cmd_tx: CmdTx,
    event_rx: Arc<EventRx>,
}

impl AsyncTcpStream {
    pub fn new(socket: Socket, cmd_tx: CmdTx, event_rx: Arc<EventRx>) -> Self {
        AsyncTcpStream {
            socket,
            cmd_tx,
            event_rx,
        }
    }
}
