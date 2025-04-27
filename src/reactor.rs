use libc::{EVFILT_READ, EVFILT_WRITE, kevent, timespec};

use crate::kqueue::Kqueue;
use std::os::fd::RawFd;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;

#[derive(Debug)]
pub enum Event {
    NewConnection(RawFd),
    Readable(RawFd),
    Writable(RawFd),
}

#[derive(Debug)]
pub enum Cmd {
    Add(RawFd, bool, bool),
    Delete(RawFd, bool),
}

pub type EventRx = mpsc::Receiver<Event>;
pub type CmdTx = mpsc::Sender<Cmd>;

#[derive(Debug)]
pub struct Reactor {
    kq: Arc<Mutex<Kqueue>>,
    listener_fd: RawFd,
}

impl Reactor {

    pub fn new(fd: RawFd) -> Result<Self, String> {
        let kq = Kqueue::new()?;
        Ok(Reactor {
            kq: Arc::new(Mutex::new(kq)),
            listener_fd: fd,
        })
    }

    pub fn start(&self) -> (CmdTx, EventRx) {
        let (cmd_tx, cmd_rx) = mpsc::channel();
        let (event_tx, event_rx) = mpsc::channel();
        let kq = self.kq.clone();
        let listener_fd = self.listener_fd;

        thread::spawn(move || {
            loop {
                
                let _ = match cmd_rx.try_recv() {
                    Ok(Cmd::Add(fd, readable, oneshot)) => {
                        kq.lock().unwrap().add(fd, readable, oneshot)
                    }
                    Ok(Cmd::Delete(fd, readable)) => kq.lock().unwrap().delete(fd, readable),
                    Err(e) => Err(e).map_err(|e| format!("failed to receive from command sender {}", e)),
                };

                let mut events = [unsafe { std::mem::zeroed::<kevent>() }; 1024];
                
                let n = kq
                    .lock()
                    .unwrap()
                    .wait(
                        &mut events,
                        Some(timespec {
                            tv_sec: 0,
                            tv_nsec: 100_000,
                        }),
                    )
                    .unwrap();

                for e in &events[..n as usize] {
                    
                    let fd = e.ident as RawFd;
                    let filter = e.filter;

                    let event = match filter {
                        EVFILT_READ if fd == listener_fd => Event::NewConnection(fd),
                        EVFILT_READ => Event::Readable(fd),
                        EVFILT_WRITE => Event::Writable(fd),
                        _ => continue,
                    };
                    // send on best effort basis
                    let _ = event_tx.send(event);
                }
            }
        });

        (cmd_tx, event_rx)
    }
}
