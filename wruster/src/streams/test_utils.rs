use std::{
    error::Error,
    io::{self, Read, Write},
    net::{Ipv4Addr, Shutdown, SocketAddrV4, TcpListener, TcpStream},
};

pub struct TcpClient {
    stream: Option<TcpStream>,
}

impl TcpClient {
    #[allow(dead_code)]
    pub fn connect(addr: String) -> Result<Self, Box<dyn Error>> {
        let stream = TcpStream::connect(&addr)?;
        let stream = Some(stream);
        Ok(TcpClient { stream })
    }

    #[allow(dead_code)]
    pub fn send(&mut self, data: &[u8]) -> Result<(), Box<dyn Error>> {
        let stream = self.stream.as_mut().unwrap();
        stream.write_all(data)?;
        stream.flush()?;
        Ok(())
    }

    pub fn close(&mut self) -> io::Result<()> {
        let stream = self.stream.as_mut().unwrap();
        stream.shutdown(Shutdown::Both)?;
        Ok(())
    }

    pub fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let stream = self.stream.as_mut().unwrap();
        stream.read(buf)
    }
}

impl Drop for TcpClient {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

impl io::Read for TcpClient {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.read(buf)
    }
}

#[allow(dead_code)]
pub fn get_free_port() -> u16 {
    let addr = SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0);
    TcpListener::bind(addr)
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}
