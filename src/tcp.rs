use std::{os::unix::io::RawFd, sync::mpsc};

use crate::{reactor::Reactor, socket::Socket};

#[derive(Debug)]
enum Event {
    NewConecction(RawFd),
    Readable(RawFd),
    Writable(RawFd),
}

#[derive(Debug)]
enum Cmd {
    Add(RawFd, bool, bool),
    Delete(RawFd),
}

type EventRx = mpsc::Receiver<Event>;
type CmdTx = mpsc::Sender<Cmd>;

#[derive(Debug)]
pub struct AsyncTcpListener {
    socket: Socket,
    reactor: Reactor,
    cmd_tx: CmdTx,
    event_rx: EventRx,
}

#[derive(Debug)]
pub struct AsyncTcpStream {
    socket: Socket,
    cmd_tx: CmdTx,
    event_rx: EventRx,
}
