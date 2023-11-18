use std::{
    error::Error,
    fs::{self, File},
    io::{self, Read, Write},
    net::{Ipv4Addr, Shutdown, SocketAddrV4, TcpListener, TcpStream},
    path::PathBuf,
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

#[allow(dead_code)]
pub fn load_test_file(name: &str) -> Result<File, io::Error> {
    let mut file_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    file_path.push("tests/assets");
    file_path.push(name);
    let file = fs::File::open(&file_path).unwrap();
    return Ok(file);
}

#[allow(dead_code)]
pub fn test_file_size(name: &str) -> Result<u64, io::Error> {
    let mut file_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    file_path.push("tests/assets");
    file_path.push(name);
    let metadata = fs::metadata(&file_path).unwrap();
    return Ok(metadata.len());
}
