use std::{
    convert::TryInto,
    error::Error,
    io::{self, BufReader, Read, Write},
    net::{Ipv4Addr, Shutdown, SocketAddr, SocketAddrV4, TcpListener, TcpStream, ToSocketAddrs},
    path::PathBuf,
    sync::Arc,
};

use rustls::{ClientConfig, ClientConnection, PrivateKey, StreamOwned};

pub fn load_test_certificate() -> io::Result<rustls::Certificate> {
    let mut cert_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    cert_path.push("tests/certs/cert.pem");
    let mut cert_reader = std::io::BufReader::new(std::fs::File::open(cert_path)?);
    let certs: Vec<rustls::Certificate> = rustls_pemfile::certs(&mut cert_reader)
        .unwrap()
        .iter()
        .map(|v| rustls::Certificate(v.clone()))
        .collect();
    Ok(certs[0].clone())
}

pub fn load_test_ca() -> io::Result<Vec<u8>> {
    let mut cert_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    cert_path.push("tests/certs/cert.der");
    let mut cert_reader = std::io::BufReader::new(std::fs::File::open(cert_path)?);
    let mut cert_contents = Vec::new();
    cert_reader.read_to_end(&mut cert_contents)?;
    Ok(cert_contents)
}

pub fn load_private_key() -> Result<PrivateKey, io::Error> {
    let mut key_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    key_path.push("tests/certs/key.pem");
    let keyfile = std::fs::File::open(key_path).expect("cannot open private key file");
    let mut reader = BufReader::new(keyfile);
    match rustls_pemfile::read_one(&mut reader)? {
        Some(rustls_pemfile::Item::RSAKey(key)) => return Ok(rustls::PrivateKey(key)),
        Some(rustls_pemfile::Item::PKCS8Key(key)) => return Ok(rustls::PrivateKey(key)),
        Some(rustls_pemfile::Item::ECKey(key)) => return Ok(rustls::PrivateKey(key)),
        None => Err(io::Error::new(
            io::ErrorKind::Other,
            "no suitable private key found",
        )),
        _ => Err(io::Error::new(
            io::ErrorKind::Other,
            "no suitable private key found",
        )),
    }
}

pub fn get_free_port() -> u16 {
    let addr = SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0);
    TcpListener::bind(addr)
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

pub fn build_tls_test_client_config() -> Result<ClientConfig, io::Error> {
    let mut root_store = rustls::RootCertStore::empty();
    let test_ca = load_test_ca()?;
    let test_cas = vec![test_ca];
    root_store.add_parsable_certificates(&test_cas);
    let suites = rustls::DEFAULT_CIPHER_SUITES;
    let versions = rustls::DEFAULT_VERSIONS.to_vec();
    let mut config = rustls::ClientConfig::builder()
        .with_cipher_suites(&suites)
        .with_safe_default_kx_groups()
        .with_protocol_versions(&versions)
        .map_err(|err| io::Error::new(io::ErrorKind::Other, err))?
        .with_root_certificates(root_store)
        .with_no_client_auth();
    config.key_log = Arc::new(rustls::KeyLogFile::new());
    Ok(config)
}

pub struct TestTLSClient {
    pub stream: StreamOwned<ClientConnection, TcpStream>,
}

impl TestTLSClient {
    pub fn new(server: &str, port: u16, config: ClientConfig) -> io::Result<TestTLSClient> {
        let addr = format!("{}:{}", server, port);
        let addrs = addr.to_socket_addrs()?;
        let addrs = addrs.collect::<Vec<SocketAddr>>();

        let server_name = server.try_into().unwrap();
        let conn = rustls::ClientConnection::new(Arc::new(config), server_name)
            .map_err(|err| io::Error::new(io::ErrorKind::Other, err))?;
        let sock = TcpStream::connect(&*addrs)?;
        let stream = StreamOwned::new(conn, sock);

        Ok(TestTLSClient { stream })
    }

    pub fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.stream.read(buf)
    }

    pub fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        self.stream.write(data)
    }
}

pub struct TcpClient {
    stream: Option<TcpStream>,
}

impl TcpClient {
    pub fn connect(addr: String) -> Result<Self, Box<dyn Error>> {
        let stream = TcpStream::connect(&addr)?;
        let stream = Some(stream);
        Ok(TcpClient { stream })
    }

    pub fn send(&mut self, data: &[u8]) -> Result<(), Box<dyn Error>> {
        let stream = self.stream.as_mut().unwrap();
        stream.write(data)?;
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
