use libc::{in_addr, sockaddr, sockaddr_in, socklen_t, AF_INET, SOCK_STREAM};
use std::{mem, net::Ipv4Addr, os::unix::io::RawFd};

unsafe extern "C" {
    // domain: Communication domain (AF_INET = IPv4).
    // type: Socket type (SOCK_STREAM = TCP).
    // protocol: Usually 0, meaning default protocol for TCP.
    // Returns a valid file descriptor (RawFd) or -1 if there's an error.
    fn socket(domain: i32, type_: i32, protocol: i32) -> i32;

    // sockfd: file descriptor for the socket
    // addr: A pointer to a socket address structure
    // addrlen The size (in bytes) of the socket address structure pointed to by addr
    fn bind(sockfd: i32, addr: *const sockaddr, addrlen: socklen_t) -> i32;

    // sockfd: raw file descriptor
    // backlog: how many pending connections can wait in the queue
    fn listen(sockfd: i32, backlog: i32) -> i32;

    // sockfd: file descriptor for the socket
    // addr: A pointer to a client socket address structure
    // addrlen The size (in bytes) of the client socket address structure pointed to by addr
    fn accept(sockfd: i32, addr: *mut sockaddr, addrlen: *mut socklen_t) -> i32;

    // connects to a remote TCP socket
    // sockfd: file descriptor for the socket
    // addr: A pointer to a client socket address structure
    // addrlen The size (in bytes) of the client socket address structure pointed to by addr
    fn connect(sockfd: i32, addr: *mut sockaddr, addrlen: *const socklen_t) -> i32;

    // sockfd: file descriptor for the socket
    // buf: a pointer to a buffer that holds the data
    // len: number of bytes in the buffer that you want to send
    // flags: behaviour of sending data: usually set to 0
    //- MSG_NOSIGNAL: Don't raise SIGPIPE signal if the peer closes the connection.
    //- MSG_DONTWAIT: Perform non-blocking operation.
    //- MSG_OOB: Send out-of-band data.
    // returns:
    // -Positive number: Number of bytes actually sent.
    // - 0: Usually means connection closed (rare in send)
    // - -1: An error occurred (errno will give details).
    fn send(sockfd: i32, buf: *const u8, len: usize, flags: i32) -> isize;

    // sockfd: file descriptor for the scoket to read data from
    // A pointer to a buffer (*mut u8) where received data will be stored.
    // Maximum length (capacity) of the buffer. Defines how many bytes you want to attempt to read.
    // Flags controlling the receiving behavior, commonly 0. Possible flags include:
    //- MSG_WAITALL: Block until the requested number of bytes are received.
    //- MSG_DONTWAIT: Perform non-blocking operation.
    //- MSG_OOB: Receive out-of-band data.
    // returns:
    //- Positive number: Number of bytes actually received and stored in buffer.
    //- 0: Connection closed gracefully by peer.
    //- -1: Error occurred (check errno).
    fn recv(sockfd: i32, buf: *mut u8, len: usize, flags: i32) -> isize;

    // closes the socket
    // fd: raw file descriptor
    fn close(fd: i32) -> i32;
}

#[derive(Debug, PartialEq)]
pub enum SocketState {
    Created,
    Bound,
    Listening,
    Connected,
    Closed,
}

pub struct Socket {
    fd: RawFd,
    state: SocketState,
}

impl Socket {
    pub fn new() -> Result<Self, String> {
        let fd = unsafe { socket(AF_INET, SOCK_STREAM, 0) };

        if fd == -1 {
            Err("Failed to create a socket".into())
        } else {
            Ok(Socket {
                fd,
                state: SocketState::Created,
            })
        }
    }

    pub fn bind(&mut self, ip: &str, port: u16) -> Result<(), String> {
        if self.state != SocketState::Created {
            return Err("Socket already bound our connected".into());
        }
        let ip: Ipv4Addr = ip.parse().map_err(|_| "Ivalid IP address")?;
        // create IPv4 address
        // TODO: make portable to support different platforms
        let addr = sockaddr_in {
            sin_len: mem::size_of::<sockaddr_in>() as u8, // length of the socket address strcut itself - only used on macOS
            sin_family: AF_INET as u8, // IPv4 address family (u8 on MacOS, u16 on Linux)
            sin_port: port.to_be(),    // port in big-endian notation
            sin_addr: in_addr {
                s_addr: u32::from(ip).to_be(),
            }, // address to bind to INADDR_ANY - all addresses 0.0.0.0
            sin_zero: [0; 8],          // padding initalized to zero's
        };

        let res = unsafe {
            bind(
                self.fd,
                &addr as *const sockaddr_in as *const sockaddr,
                mem::size_of::<sockaddr_in>() as u32,
            )
        };

        if res == -1 {
            return Err("Failed to bind socket".into());
        }

        self.state = SocketState::Bound;
        Ok(())
    }

    pub fn listen(&mut self, backlog: i32) -> Result<(), String> {
        if self.state != SocketState::Bound {
            return Err("Socket must be bound before listening".into());
        }

        let res = unsafe { listen(self.fd, backlog) };

        if res == -1 {
            return Err("Failed to listen on socket".into());
        }

        self.state = SocketState::Listening;
        Ok(())
    }

    pub fn accept(&self) -> Result<Socket, String> {
        if self.state != SocketState::Listening {
            return Err("Socket is not listening".into());
        }

        let client_fd = unsafe { accept(self.fd, std::ptr::null_mut(), std::ptr::null_mut()) };

        if client_fd == -1 {
            return Err("Failed to accept connection".into());
        }

        Ok(Socket {
            fd: client_fd,
            state: SocketState::Connected,
        })
    }
}

impl Drop for Socket {
    fn drop(&mut self) {
        if self.state != SocketState::Closed {
            unsafe { close(self.fd); }
            self.state = SocketState::Closed;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_can_create_socket() {
        let sock = Socket::new();
        assert_eq!(
            sock.is_ok(),
            true,
            "retured a file descriptor with a value of -1"
        );
    }

    #[test]
    fn test_bind_socket_to_port() {
        let mut sock = Socket::new().expect("Failed to create socket");
        // use 0 to allow the use to chose an avaiable ephepermal port
        let _ = sock.bind("0.0.0.0", 0);
        // close the socket after use
        unsafe {
            close(sock.fd);
        }
    }

    #[test]
    fn test_bind_socket_invalid_fd() {
        // passing invalid socket descriptor
        let mut sock = Socket::new().expect("Failed to create socket");
        // use 0 to allow the use to chose an avaiable ephepermal port
        let res = sock.bind("-dvddfvfdvdvd0.0.0.0", 0);

        assert_eq!(res.is_err(), true, "Should fail to bind scoket")
    }

    #[test]
    fn test_bind_socket_port_in_use() {
        let mut sock_1 = Socket::new().expect("Failed to create socket");
        let mut sock_2 = Socket::new().expect("Failed to create socket");

        // bind first soccket
        let res1 = sock_1.bind("0.0.0.0", 1150);
        // bind second sock to the same port
        let res2 = sock_2.bind("0.0.0.0", 1150);

        assert_eq!(res1.is_ok(), true, "Failed to bind socket to port");
        assert_ne!(res2.is_ok(), true, "Bound socket to port successfully");

        unsafe {
            close(sock_1.fd);
            close(sock_2.fd);
        }
    }
}
