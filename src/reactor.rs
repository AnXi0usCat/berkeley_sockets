use libc::{EVFILT_READ, EVFILT_WRITE, kevent, timespec};

use crate::kqueue::Kqueue;
use std::collections::HashMap;
use std::os::fd::RawFd;
use std::sync::{Arc, Mutex, mpsc};
use std::task::Waker;
use std::thread::{self, JoinHandle};

#[derive(Debug, Clone)]
pub enum Event {
    NewConnection(RawFd),
    Readable(RawFd),
    Writable(RawFd),
}

#[derive(Debug, Clone)]
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
    wakers: Arc<Mutex<HashMap<(RawFd, i16), Waker>>>,
    event_loop: Option<JoinHandle<()>>,
}

impl Reactor {
    pub fn new(fd: RawFd) -> Result<Self, String> {
        let kq = Kqueue::new()?;
        let mut reactor = Reactor {
            kq: Arc::new(Mutex::new(kq)),
            listener_fd: fd,
            wakers: Arc::new(Mutex::new(HashMap::new())),
            event_loop: None,
        };

        reactor.event_loop();
        Ok(reactor)
    }

    pub fn register(
        &self,
        fd: RawFd,
        readable: bool,
        oneshot: bool,
        waker: &Waker,
    ) -> Result<(), String> {
        let filter = if readable {
            libc::EVFILT_READ
        } else {
            libc::EVFILT_WRITE
        };
        self.wakers
            .lock()
            .unwrap()
            .insert((fd, filter), waker.clone());
        self.kq.lock().unwrap().add(fd, readable, oneshot)
    }

    pub fn event_loop(&mut self) {
        let kq = Arc::clone(&self.kq);
        let wakers = Arc::clone(&self.wakers);

        let handle = thread::spawn(move || {
            let mut events = [unsafe { std::mem::zeroed::<kevent>() }; 1024];
            loop {
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
                    if let Some(waker) = wakers.lock().unwrap().remove(&(fd, filter)) {
                        waker.wake();
                    }
                }
            }
        });

        self.event_loop = Some(handle);
    }
}
