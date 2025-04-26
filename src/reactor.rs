use crate::kqueue::Kqueue;
use std::os::fd::RawFd;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;

#[derive(Debug)]
pub enum Event {
    NewConecction(RawFd),
    Readable(RawFd),
    Writable(RawFd),
}

#[derive(Debug)]
pub enum Cmd {
    Add(RawFd, bool, bool),
    Delete(RawFd),
}

pub type EventRx = mpsc::Receiver<Event>;
pub type CmdTx = mpsc::Sender<Cmd>;

#[derive(Debug)]
pub struct Reactor {
    kq: Arc<Mutex<Kqueue>>,
}

impl Reactor {
    pub fn new() -> Result<Self, String> {
        let kq = Kqueue::new()?;
        Ok(Reactor {
            kq: Arc::new(Mutex::new(kq)),
        })
    }

    pub fn start(&self) -> (CmdTx, EventRx) {
        let (cmd_tx, cmd_rx) = mpsc::channel();
        let (event_tx, event_rx) = mpsc::channel();
        let kq_reactor = self.kq.clone();

        thread::spawn(move || {
            Reactor::_start(kq_reactor, cmd_rx, event_tx);
        });

        (cmd_tx, event_rx)
    }

    fn _start(kq: Arc<Mutex<Kqueue>>, cmd_rx: Receiver<Cmd>, event_tx: Sender<Event>) {}
}
