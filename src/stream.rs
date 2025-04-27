use crate::reactor::{CmdTx, EventRx};
use crate::{
    reactor::{Cmd, Reactor},
    socket::Socket,
};

#[derive(Debug)]
pub struct AsyncTcpListener {
    socket: Socket,
    reactor: Reactor,
    cmd_tx: CmdTx,
    event_rx: EventRx,
}

impl AsyncTcpListener {
    
    pub fn bind(host: &str, port: u16) -> Result<Self, String> {
        let mut sc = Socket::new()?;
        sc.bind(host, port)?;
        sc.set_nonblocking(true)?;
        sc.listen(100)?;

        let reactor = Reactor::new()?;
        let (tx, rx) = reactor.start();

        tx.send(Cmd::Add(sc.fd, true, false))
            .map_err(|_| "failed to add listener to kqueue")?;

        Ok(AsyncTcpListener {
            socket: sc,
            reactor,
            cmd_tx: tx,
            event_rx: rx,
        })
    }
}

#[derive(Debug)]
pub struct AsyncTcpStream {
    socket: Socket,
    cmd_tx: CmdTx,
    event_rx: EventRx,
}
