use super::*;
use std::io::BufReader;

#[test]
fn http_header_parse_standard() {
    // Test parsing standard header.
    let header_content = "header-name:headervalue\r\n";
    let mut stream = BufReader::new(header_content.as_bytes());
    let header_content = HttpHeader::read_from(&mut stream).unwrap().unwrap();
    assert_eq!(header_content.field_name.as_str(), "header-name");
    assert_eq!(header_content.field_content.as_str(), "header-value");
}

#[test]
fn http_header_invalid_tokens() {
    // Only accepts valid chars in tokens.
    let header_content = "header\rname:headervalue\r\n";
    let mut stream = BufReader::new(header_content.as_bytes());
    assert_eq!(
        HttpHeader::read_from(&mut stream).unwrap_err(),
        ParseRequestError {
            msg: String::from("invalid header value")
        }
    );


    let header_content = "header\x0Bname:headervalue\r\n";
    let mut stream = BufReader::new(header_content.as_bytes());
    assert_eq!(
        HttpHeader::read_from(&mut stream).unwrap_err(),
        ParseRequestError {
            msg: String::from("invalid header value")
        }
    );
}

#[test]
fn http_header_no_colon() {
    let header_content = "header-name :headervalue\r\n";
    let mut stream = BufReader::new(header_content.as_bytes());
    assert_eq!(
        HttpHeader::read_from(&mut stream).unwrap_err(),
        ParseRequestError {
            msg: String::from("invalid header name")
        }
    );
}

#[test]
fn http_header_invalid_header_values() {
    let header_content = "header-name: \0x1Aheadervalue\r\n";
    let mut stream = BufReader::new(header_content.as_bytes());
    assert_eq!(
        HttpHeader::read_from(&mut stream).unwrap_err(),
        ParseRequestError {
            msg: String::from("invalid header value")
        }
    );
}
