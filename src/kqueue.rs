use std::os::unix::io::RawFd;

use libc::{kevent as kevent_struct, kqueue, timespec};

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
        fdesc: RawFd,
        changelist: *const kevent_struct,
        nchanges: i32,
        eventlist: *mut kevent_struct,
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

#[derive(Debug)]
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
