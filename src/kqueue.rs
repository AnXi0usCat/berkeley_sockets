use std::{os::unix::io::RawFd, usize};

use libc::{
    c_void, kevent as kevent_struct, timespec, uintptr_t, EVFILT_READ, EVFILT_WRITE, EV_ADD,
    EV_CLEAR, EV_DELETE, EV_ONESHOT,
};

unsafe extern "C" {
    // system call	creates	a new kernel event queue and returns a
    // descriptor.   The  queue	 is  not  inherited  by	 a  child created with
    // fork(2).	 However, if rfork(2) is called	without	the RFFDG  flag,  then
    // the  descriptor table is	shared,	which will allow sharing of the	kqueue
    // between two processes.
    fn kqueue() -> i32;

    // system call	is used	to register events with	the queue, and
    // return any pending events to the	user.  The changelist  argument	 is  a
    // pointer	to an array of kevent structures, as defined in	<sys/event.h>.
    // All changes contained in	the changelist are applied before any  pending
    // events  are  read from the queue.  The nchanges argument	gives the size
    // of changelist.  The eventlist argument is a  pointer  to	 an  array  of
    // kevent  structures.   The  nevents  argument  determines	 the  size  of
    // eventlist.  When	nevents	is zero, kevent() will return immediately even
    // if there	is a timeout specified unlike select(2).  If timeout is	a non-
    // NULL pointer, it	specifies a maximum interval to	 wait  for  an	event,
    // which  will  be interpreted as a	struct timespec.  If timeout is	a NULL
    // pointer,	kevent() waits indefinitely.  To effect	a  poll,  the  timeout
    // argument	 should	be non-NULL, pointing to a zero-valued timespec	struc-
    // ture.  The same array may be used for the changelist and	eventlist.
    // struct kevent {
    //     uintptr_t ident;	       /* identifier for this event */
    //     short	 filter;       /* filter for event */
    //     u_short	 flags;	       /* action flags for kqueue */
    //     u_int	 fflags;       /* filter flag value */
    //     intptr_t	 data;	       /* filter data value */
    //     void	 *udata;           /* opaque user data identifier */
    // };
    fn kevent(
        kq: RawFd,
        changelist: *const kevent_struct,
        nchanges: i32,
        eventlist: *const kevent_struct,
        nevents: i32,
        timeout: *const timespec,
    ) -> i32;

    // closes the file descriptor
    // fd: raw file descriptor
    fn close(fd: i32) -> i32;

    // access to the thread local errno variable which
    // should have the latest error code set to it
    fn __error() -> *mut libc::c_int;
}

#[derive(Debug, PartialEq)]
struct Kqueue {
    kq: RawFd,
}

impl Kqueue {
    pub fn new() -> Result<Kqueue, String> {
        let fd = unsafe { kqueue() };

        if fd < 0 {
            return Err(format!(
                "Failed to create kqueue() file descriptor: {}",
                Kqueue::errno()
            ));
        }

        Ok(Kqueue { kq: fd })
    }

    pub fn add(&self, fd: RawFd, readable: bool, oneshot: bool) -> Result<(), String> {
        // readable == true -> EVFILT_READ, readble == false -> EVFILT_WRITE
        let filter = if readable { EVFILT_READ } else { EVFILT_WRITE };

        // EV_ADD = 0x0001 (register this event)
        // EV_CLEAR = 0x0020 (edge‑triggered: after you get an event, you must drain it to avoid missing future ones)
        // EV_ONESHOT= 0x0010 (automatically delete this event after it fires once)
        //
        // when oneshot == false:
        //
        // flags = 0x0001 /*EV_ADD*/
        //       | 0x0020 /*EV_CLEAR*/
        //       | 0x0000
        //       = 0x0021
        //
        // when oneshot == true:
        //
        // flags = 0x0001 /*EV_ADD*/
        //       | 0x0020 /*EV_CLEAR*/
        //       | 0x0010 /*EV_ONESHOT*/
        //       = 0x0031
        let flags = EV_ADD | EV_CLEAR | if oneshot { EV_ONESHOT } else { 0 };

        let event = kevent_struct {
            ident: fd as uintptr_t,
            filter,
            flags,
            fflags: 0,
            data: 0,
            udata: std::ptr::null_mut() as *mut c_void,
        };

        let res = unsafe {
            kevent(
                self.kq,
                &event as *const kevent_struct,
                1,
                std::ptr::null(),
                0,
                std::ptr::null() as *const timespec,
            )
        };

        if res < 0 {
            return Err(format!(
                "Failed to ADD event to kevent(): {}",
                Kqueue::errno()
            ));
        }

        Ok(())
    }

    pub fn delete(&self, fd: RawFd, readable: bool) -> Result<(), String> {
        // readable == true -> EVFILT_READ, readble == false -> EVFILT_WRITE
        let filter = if readable { EVFILT_READ } else { EVFILT_WRITE };

        let event = kevent_struct {
            ident: fd as uintptr_t,
            filter,
            flags: EV_DELETE,
            fflags: 0,
            data: 0,
            udata: std::ptr::null_mut() as *mut c_void,
        };

        let res = unsafe {
            kevent(
                self.kq,
                &event as *const kevent_struct,
                1,
                std::ptr::null(),
                0,
                std::ptr::null() as *const timespec,
            )
        };

        if res < 0 {
            return Err(format!(
                "Failed to DELETE event from kevent(): {}",
                Kqueue::errno()
            ));
        }

        Ok(())
    }

    pub fn wait(
        &self,
        events: &[kevent_struct],
        timeout: Option<timespec>,
    ) -> Result<usize, String> {
        let p_timeout = match timeout {
            Some(ts) => &ts as *const timespec,
            None => std::ptr::null() as *const timespec,
        };

        let n = unsafe {
            kevent(
                self.kq,
                std::ptr::null(),
                0,
                events.as_ptr(),
                events.len() as i32,
                p_timeout,
            )
        };

        if n < 0 {
            return Err(format!(
                "Failed to WAIT on events in kevent(): {}",
                Kqueue::errno()
            ));
        }

        Ok(n as usize)
    }

    fn errno() -> i32 {
        unsafe { *__error() }
    }
}

impl Drop for Kqueue {
    fn drop(&mut self) {
        unsafe {
            close(self.kq);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Kqueue;
    use libc::{kevent, pipe, read, timespec, write};
    use std::{mem, os::fd::RawFd};

    #[test]
    fn test_can_create_kq() {
        let kq = Kqueue::new().expect("failed");
        assert_eq!(kq, Kqueue { kq: kq.kq });
    }

    #[test]
    fn test_can_register_fd() {
        // GIVEN
        let mut fds = [0; 2];
        let res = unsafe { pipe(fds.as_mut_ptr()) };
        assert_eq!(res, 0);

        let read_fd = fds[0];
        let write_fd = fds[1];

        // WHEN
        let kq = Kqueue::new().expect("failed");
        kq.add(read_fd, true, false).expect("failed to add");

        // write something to the wrtie end of the pipe
        const MESSAGE: &[u8] = b"hello";
        let written = unsafe {
            write(
                write_fd,
                MESSAGE.as_ptr() as *const libc::c_void,
                MESSAGE.len(),
            )
        };
        assert_eq!(written as usize, MESSAGE.len());

        // THEN
        let events = [unsafe { mem::zeroed() }; 1];
        let n = kq.wait(&events, None).unwrap();
        assert_eq!(n, 1);
    }

    #[test]
    fn test_can_wait_on_data() {
        // GIVEN
        let mut fds = [0; 2];
        let res = unsafe { pipe(fds.as_mut_ptr()) };
        assert_eq!(res, 0);

        let read_fd = fds[0];
        let write_fd = fds[1];

        // WHEN
        let kq = Kqueue::new().expect("failed");
        kq.add(read_fd, true, false).expect("failed to add");

        // write something to the wrtie end of the pipe
        const MESSAGE: &[u8] = b"hello";
        let written = unsafe {
            write(
                write_fd,
                MESSAGE.as_ptr() as *const libc::c_void,
                MESSAGE.len(),
            )
        };
        assert_eq!(written as usize, MESSAGE.len());

        let events = [unsafe { mem::zeroed() }; 1];
        let n = kq.wait(&events, None).unwrap();
        assert_eq!(n, 1);

        let mut buf = [0u8; MESSAGE.len()];
        let n = unsafe {
            read(
                read_fd,
                buf.as_mut_ptr() as *mut libc::c_void,
                MESSAGE.len(),
            )
        };
        // THEN
        assert_eq!(n as usize, MESSAGE.len());
        assert_eq!(&buf[..n as usize], MESSAGE);
    }

    #[test]
    fn test_wait_timeout() {
        // GIVEN
        let kq = Kqueue::new().expect("failed");
        let events = [unsafe { mem::zeroed() }; 1];

        // WHEN
        let n = kq
            .wait(
                &events,
                Some(timespec {
                    tv_sec: 0,
                    tv_nsec: 1000,
                }),
            )
            .unwrap();
        // THEN
        assert_eq!(n, 0);
    }

    #[test]
    fn test_can_delete_from_kqueue() {
        // GIVEN
        let mut fds = [0; 2];
        let res = unsafe { pipe(fds.as_mut_ptr()) };
        assert_eq!(res, 0);

        let read_fd = fds[0];
        let write_fd = fds[1];

        // WHEN
        let kq = Kqueue::new().expect("failed");
        kq.add(read_fd, true, false).expect("failed to add");
        kq.delete(read_fd, true).expect("delete failed");

        // write something to the wrtie end of the pipe
        const MESSAGE: &[u8] = b"hello";
        let written = unsafe {
            write(
                write_fd,
                MESSAGE.as_ptr() as *const libc::c_void,
                MESSAGE.len(),
            )
        };
        assert_eq!(written as usize, MESSAGE.len());

        let events = [unsafe { mem::zeroed() }; 1];
        let n = kq
            .wait(
                &events,
                Some(timespec {
                    tv_sec: 0,
                    tv_nsec: 1000,
                }),
            )
            .unwrap();
        // THEN
        assert_eq!(n, 0);
    }
}
