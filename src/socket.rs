use libc::{in_addr, sockaddr, sockaddr_in, socklen_t, AF_INET, INADDR_ANY, SOCK_STREAM};
use std::{mem, os::unix::io::RawFd};

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

    // closes the socket
    // fd: raw file descriptor
    fn close(fd: i32) -> i32;
}

fn create_socket() -> RawFd {
    let fd = unsafe { socket(AF_INET, SOCK_STREAM, 0) };

    if fd == -1 {
        panic!("Failed to create a socket");
    }

    println!("Socket created successfully! FD: {}", fd);
    fd
}

fn bind_socket(fd: RawFd, port: u16) {
    // create IPv4 address
    // TODO: make portable to support different platforms
    let addr = sockaddr_in {
        sin_len: mem::size_of::<sockaddr_in>() as u8, // length of the socket address strcut itself - only used on macOS
        sin_family: AF_INET as u8, // IPv4 address family (u8 on MacOS, u16 on Linux)
        sin_port: port.to_be(),    // port in big-endian notation
        sin_addr: in_addr { s_addr: INADDR_ANY }, // address to bind to INADDR_ANY - all addresses 0.0.0.0
        sin_zero: [0; 8],                         // padding initalized to zero's
    };

    let res = unsafe {
        bind(
            fd,
            &addr as *const sockaddr_in as *const sockaddr,
            mem::size_of::<sockaddr_in>() as u32,
        )
    };

    if res == -1 {
        panic!("Failed to bind the socket");
    }

    println!("Socket bound successfully to port: {}", port);
}

fn listen_socket(fd: RawFd) {
    let res = unsafe { listen(fd, 10) };

    if res == -1 {
        panic!("Failed to listen on socket");
    }

    println!("Socket is now listening");
}

fn accept_connection(fd: RawFd) -> RawFd {
    let mut client_addr: sockaddr_in = unsafe { mem::zeroed() };
    let mut addr_len: u32 = mem::size_of::<sockaddr_in>() as u32;

    let client_fd = unsafe {
        accept(
            fd,
            &mut client_addr as *mut sockaddr_in as *mut sockaddr,
            &mut addr_len as *mut u32,
        )
    };

    if client_fd == -1 {
        panic!("Failed to accept connection");
    }

    println!("Connection accepted! CLient FD: {}", client_fd);
    client_fd
}

fn close_socket(fd: RawFd) {
    unsafe {
        close(fd);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_can_create_socket() {
        let fd = create_socket();
        assert_ne!(fd, -1, "retured a file descriptor with a value of -1");
    }

    #[test]
    fn test_bind_socket_to_port() {
        let fd = create_socket();
        // use 0 to allow the use to chose an avaiable ephepermal port
        bind_socket(fd, 0);
        // close the socket after use
        unsafe {
            close(fd);
        }
    }

    #[test]
    #[should_panic(expected = "Failed to bind the socket")]
    fn test_bind_socket_invalid_fd() {
        // passing invalid socket descriptor
        bind_socket(-1, 0);
    }

    #[test]
    #[should_panic(expected = "Failed to bind the socket")]
    fn test_bind_socket_port_in_use() {
        let fd1 = create_socket();
        let fd2 = create_socket();

        // bind first soccket
        bind_socket(fd1, 1150);

        // bind second sock to the same port
        bind_socket(fd2, 1150);

        unsafe {
            close(fd1);
            close(fd2);
        }
    }
}
