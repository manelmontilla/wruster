use std::{
    convert::TryInto,
    io::{self, Read, Write},
    net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpListener, TcpStream, ToSocketAddrs},
    path::PathBuf,
    sync::Arc,
};

use rustls::{ClientConfig, ClientConnection, StreamOwned};

pub use crate::streams::tls::test_utils::{load_test_certificate, load_test_private_key};

/**
 Returns a free TCP port
*/
pub fn get_free_port() -> u16 {
    let addr = SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0);
    TcpListener::bind(addr)
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn build_tls_test_client_config() -> Result<ClientConfig, io::Error> {
    let mut root_store = rustls::RootCertStore::empty();
    let test_ca = load_test_ca()?;
    let test_cas: Vec<Vec<u8>> = vec![test_ca];
    root_store.add_parsable_certificates(&test_cas);
    let suites = rustls::DEFAULT_CIPHER_SUITES;
    let versions = rustls::DEFAULT_VERSIONS.to_vec();
    let mut config = rustls::ClientConfig::builder()
        .with_cipher_suites(suites)
        .with_safe_default_kx_groups()
        .with_protocol_versions(&versions)
        .map_err(|err| io::Error::new(io::ErrorKind::Other, err))?
        .with_root_certificates(root_store)
        .with_no_client_auth();
    config.key_log = Arc::new(rustls::KeyLogFile::new());
    Ok(config)
}

/**
    Implements a TCP client that uses TLS connections suitable to be used in tests.
*/
pub struct TestTLSClient {
    /**
     The TLS stream used by the client.
    */
    pub stream: StreamOwned<ClientConnection, TcpStream>,
}

impl TestTLSClient {
    /**
    Returns a [TestTLSClient] connected to address in the specified host:port.
    */
    pub fn new(host: &str, port: u16) -> io::Result<TestTLSClient> {
        let addr = format!("{}:{}", host, port);
        let addrs = addr.to_socket_addrs()?;
        let addrs = addrs.collect::<Vec<SocketAddr>>();

        let server_name = host.try_into().unwrap();
        let config = build_tls_test_client_config()?;
        let conn = rustls::ClientConnection::new(Arc::new(config), server_name)
            .map_err(|err| io::Error::new(io::ErrorKind::Other, err))?;
        let sock = TcpStream::connect(&*addrs)?;
        let stream = StreamOwned::new(conn, sock);

        Ok(TestTLSClient { stream })
    }

    /**
    reads from the server the [TestTLSClient] is connected to.
    */
    pub fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.stream.read(buf)
    }

    /**
    writes to the server the [TestTLSClient] is connected to.
    */
    pub fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        self.stream.write(data)
    }
}

fn load_test_ca() -> io::Result<Vec<u8>> {
    let mut cert_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    cert_path.push("tests/certs/cert.der");
    let mut cert_reader = std::io::BufReader::new(std::fs::File::open(cert_path)?);
    let mut cert_contents = Vec::new();
    cert_reader.read_to_end(&mut cert_contents)?;
    Ok(cert_contents)
}
