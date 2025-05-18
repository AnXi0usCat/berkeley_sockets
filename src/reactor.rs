use libc::{kevent, timespec};

use crate::kqueue::Kqueue;
use std::collections::HashMap;
use std::os::fd::RawFd;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::task::Waker;
use std::thread::{self, JoinHandle};

#[derive(Debug)]
pub struct Reactor {
    kq: Arc<Mutex<Kqueue>>,
    wakers: Arc<Mutex<HashMap<(RawFd, i16), Waker>>>,
    event_loop: Option<JoinHandle<()>>,
    interrupt: Arc<AtomicBool>,
}

impl Reactor {
    pub fn new() -> Result<Self, String> {
        let kq = Kqueue::new()?;
        let mut reactor = Reactor {
            kq: Arc::new(Mutex::new(kq)),
            wakers: Arc::new(Mutex::new(HashMap::new())),
            event_loop: None,
            interrupt: Arc::new(AtomicBool::new(false)),
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

    fn event_loop(&mut self) {
        let kq = Arc::clone(&self.kq);
        let wakers = Arc::clone(&self.wakers);
        let interrupt = Arc::clone(&self.interrupt);

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

                if interrupt.load(Ordering::Acquire) {
                    break;
                }
            }
        });

        self.event_loop = Some(handle);
    }
}

impl Drop for Reactor {
    fn drop(&mut self) {
        self.interrupt.store(true, Ordering::Release);
        if let Some(handle) = self.event_loop.take() {
            handle.join().unwrap();
        }
    }
}
