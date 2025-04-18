use libc::{
    in_addr, sockaddr, sockaddr_in, socklen_t, AF_INET, EAGAIN, EWOULDBLOCK, F_GETFL, F_SETFL,
    O_NONBLOCK, SOCK_STREAM,
};
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
    fn connect(sockfd: i32, addr: *const sockaddr, addrlen: *const socklen_t) -> i32;

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

    // performs operations on the file descriptor
    // sokcfd: a raw file descriptor for the socket
    // op: to perform on a file descriptor
    // flags: optional arguments that correspond to the operation
    // in our case we want to set a nonblocking flag on the
    // socket file descriptor
    fn fcntl(sockfd: i32, op: i32, flags: i32) -> i32;

    // closes the socket
    // fd: raw file descriptor
    fn close(fd: i32) -> i32;

    // access to the thread local errno variable which
    // should have the latest error code set to it
    fn __error() -> *mut libc::c_int;

    // get the socket address to which the exisitng socket is bound
    // addr: A pointer to a client socket address structure
    // addrlen The size (in bytes) of the client socket address structure pointed to by addr
    fn getsockname(sockfd: i32, addr: *mut sockaddr, addrlen: *mut socklen_t) -> i32;
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

    pub fn accept_nonblocking(&self) -> Result<Option<Socket>, String> {
        if self.state != SocketState::Listening {
            return Err("Socket is not listening".into());
        }

        let client_fd = unsafe { accept(self.fd, std::ptr::null_mut(), std::ptr::null_mut()) };

        if client_fd < 0 {
            let err = unsafe { *__error() };
            if err == EAGAIN || err == EWOULDBLOCK {
                // connecction not yet available, will block
                return Ok(None);
            } else {
                return Err("Failed to accept connection".into());
            }
        }

        Ok(Some(Socket {
            fd: client_fd,
            state: SocketState::Connected,
        }))
    }

    pub fn connect(&mut self, ip: &str, port: u16) -> Result<(), String> {
        if self.state != SocketState::Created {
            return Err("Socket already bound or connected".into());
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
            connect(
                self.fd,
                &addr as *const sockaddr_in as *const sockaddr,
                mem::size_of::<sockaddr_in>() as *const u32,
            )
        };

        if res == -1 {
            return Err("Failed to connect to address".into());
        }

        self.state = SocketState::Connected;
        Ok(())
    }

    pub fn send(&self, data: &[u8]) -> Result<usize, String> {
        if self.state != SocketState::Connected {
            return Err("Socket not connected".into());
        }

        let bytes_sent = unsafe { send(self.fd, data.as_ptr(), data.len(), 0) };

        if bytes_sent < 0 {
            return Err("Failed to send data".into());
        }

        Ok(bytes_sent as usize)
    }

    pub fn send_nonblocking(&self, data: &[u8]) -> Result<Option<usize>, String> {
        if self.state != SocketState::Connected {
            return Err("Socket not connected".into());
        }

        let bytes_sent = unsafe { send(self.fd, data.as_ptr(), data.len(), 0) };

        if bytes_sent < 0 {
            let err = unsafe { *__error() };
            if err == EAGAIN || err == EWOULDBLOCK {
                return Ok(None);
            } else {
                return Err("Failed to send data to the socket".into());
            }
        }

        Ok(Some(bytes_sent as usize))
    }

    pub fn recieve(&self, buffer: &mut [u8]) -> Result<usize, String> {
        if self.state != SocketState::Connected {
            return Err("Socket not connected".into());
        }

        let bytes_received = unsafe { recv(self.fd, buffer.as_mut_ptr(), buffer.len(), 0) };

        if bytes_received < 0 {
            return Err("Receive failed".into());
        }

        Ok(bytes_received as usize)
    }

    pub fn recieve_nonblocking(&self, buffer: &mut [u8]) -> Result<Option<usize>, String> {
        if self.state != SocketState::Connected {
            return Err("Socket not connected".into());
        }

        let bytes_received = unsafe { recv(self.fd, buffer.as_mut_ptr(), buffer.len(), 0) };

        if bytes_received < 0 {
            let err = unsafe { *__error() };
            if err == EAGAIN || err == EWOULDBLOCK {
                // no data is avaialble for us to recieve yet
                return Ok(None);
            } else {
                return Err("Failed to receive data".into());
            }
        } else if bytes_received == 0 {
            return Err("Connection close by peer".into());
        }

        Ok(Some(bytes_received as usize))
    }

    pub fn set_nonblocking(&self, nonblocking: bool) -> Result<(), String> {
        // get the current flags
        let flags = unsafe { fcntl(self.fd, F_GETFL, 0) };

        if flags < 0 {
            return Err("Could not get the fule descriptor flags".into());
        }

        let new_flags = if nonblocking {
            flags | O_NONBLOCK
        } else {
            flags & !O_NONBLOCK
        };

        // set the new flags
        let res = unsafe { fcntl(self.fd, F_SETFL, new_flags) };

        if res < 0 {
            return Err("Failed to set the socket in non blocking mode".into());
        }

        Ok(())
    }

    pub fn get_assigned_port(&self) -> Result<usize, String> {
        // return the value of type represented with an all 0 byte pattern
        let mut addr: sockaddr_in = unsafe { mem::zeroed() };
        // ge tthe size of the struct and cast it to a corresponding libc type
        let mut len = mem::size_of::<sockaddr_in>() as socklen_t;

        let res = unsafe {
            getsockname(
                self.fd,
                &mut addr as *mut sockaddr_in as *mut sockaddr,
                &mut len as *mut socklen_t,
            )
        };

        if res < 0 {
            return Err("Failed to get the socket address.".into());
        }

        Ok(u16::from_be(addr.sin_port) as usize)
    }
}

impl Drop for Socket {
    fn drop(&mut self) {
        if self.state != SocketState::Closed {
            unsafe {
                close(self.fd);
            }
            self.state = SocketState::Closed;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{thread, time::Duration};

    use super::*;

    fn get_ssigned_port() {}

    fn create_server(host: &str, port: u16) -> Socket {
        let mut server_sock = Socket::new().expect("Failed to create socket.");
        server_sock
            .bind(host, port)
            .expect("Failed to bind to address.");
        server_sock
            .listen(5)
            .expect("Failed to listen to connections.");

        println!("Listening at {}:{}", host, port);
        server_sock
    }

    fn create_client(host: &str, port: u16) -> Socket {
        let mut client_sock = Socket::new().expect("Failed to create socket");
        client_sock.connect(host, port).expect("Failed to connect.");

        client_sock
    }

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
        let res = sock.bind("", 0);

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

    #[test]
    fn test_can_connect() {
        thread::scope(|sc| {
            let server = create_server("127.0.0.1", 0);
            let port = server.get_assigned_port().expect("failed to get port");

            sc.spawn(move || {
                let mut client = Socket::new().expect("Failed to create socket");
                let res = client.connect("127.0.0.1", port as u16);
                assert_eq!(Ok(()), res);
                drop(client);
            });

            let sock = server.accept().expect("Failed to accept");
            drop(server);
            drop(sock);
        });
    }

    #[test]
    fn test_can_listen() {
        let mut sock = Socket::new().expect("Failed to create socket.");
        sock.bind("127.0.0.1", 0)
            .expect("Failed to bind to address.");
        let res = sock.listen(1);

        assert_eq!(Ok(()), res);
    }

    #[test]
    fn test_send_receive() {
        thread::scope(|sc| {
            let server = create_server("127.0.0.1", 0);
            let port = server.get_assigned_port().expect("Failed to get port");

            sc.spawn(move || {
                let client = create_client("127.0.0.1", port as u16);
                let message = b"Hello Server";
                client.send(message).expect("Send failed");
            });

            let client_sock = server.accept().expect("Failed to accept");

            let mut buf = [0u8; 1024];
            let recevied = client_sock.recieve(&mut buf).expect("Receive failed");
            assert_eq!(String::from_utf8_lossy(&buf[..recevied]), "Hello Server");
        });
    }

    #[test]
    fn test_set_non_blocking_true() {
        let sock = Socket::new().expect("Failed to create socket.");
        let res = sock.set_nonblocking(true);
        assert_eq!(res, Ok(()));
    }

    #[test]
    fn test_set_non_blocking_false() {
        let sock = Socket::new().expect("Failed to create socket.");
        let res = sock.set_nonblocking(false);
        assert_eq!(res, Ok(()));
    }

    #[test]
    fn test_accept_nonblocking_no_connections() {
        let mut sock = Socket::new().expect("Failed to create socket.");
        sock.bind("127.0.0.1", 0)
            .expect("failed to bind to address");
        sock.set_nonblocking(true)
            .expect("Faild to set non blocking");
        sock.listen(5).expect("Failed to listen");
        let res = sock.accept_nonblocking().unwrap();

        assert_eq!(res.is_none(), true);
    }

    #[test]
    fn test_accept_nonblocking_get_connection() {
        let mut sock = Socket::new().expect("Failed to create socket.");
        sock.bind("127.0.0.1", 0)
            .expect("failed to bind to address");
        sock.set_nonblocking(true)
            .expect("Faild to set non blocking");
        sock.listen(5).expect("Failed to listen");

        let port = sock.get_assigned_port().expect("Failed to get port");

        thread::spawn(move || {
            let client = create_client("127.0.0.1", port as u16);
            drop(client);
        });
        loop {
            if let Ok(value) = sock.accept_nonblocking() {
                if value.is_some() {
                    break;
                };
            } else {
                thread::sleep(Duration::from_micros(100));
            }
        }
    }

    #[test]
    fn test_can_get_socket_port() {
        let mut sock = Socket::new().expect("Failed to create socket.");
        sock.bind("127.0.0.1", 8098)
            .expect("failed to bind to address");

        assert_eq!(sock.get_assigned_port().unwrap(), 8098);
    }
}
