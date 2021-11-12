use std::io::BufReader;
use std::iter::FromIterator;

use super::*;

#[test]
fn http_header_parse_standard() {
    // Test parsing standard headers
    let header_content = "header-name:header value\r\n";
    let mut stream = BufReader::new(header_content.as_bytes());
    let header_content = HttpHeader::read_from(&mut stream).unwrap().unwrap();
    assert_eq!(header_content.field_name.as_str(), "header-name");
    assert_eq!(header_content.field_content.as_str(), "header value");
}

#[test]
fn http_header_parse_with_colon_values() {
    let header_content = "Host: localhost:1234\r\n";
    let mut stream = BufReader::new(header_content.as_bytes());
    let header_content = HttpHeader::read_from(&mut stream).unwrap().unwrap();
    assert_eq!(header_content.field_name.as_str(), "Host");
    assert_eq!(header_content.field_content.as_str(), "localhost:1234");
}

#[test]
fn http_header_invalid_tokens() {
    // Only accepts valid chars in tokens.
    let header_content = "header\rname:headervalue\r\n";
    let mut stream = BufReader::new(header_content.as_bytes());
    assert_eq!(
        HttpHeader::read_from(&mut stream).unwrap_err(),
        ParseRequestError {
            msg: String::from("invalid header name")
        }
    );

    let header_content = "header\x0Bname:headervalue\r\n";
    let mut stream = BufReader::new(header_content.as_bytes());
    assert_eq!(
        HttpHeader::read_from(&mut stream).unwrap_err(),
        ParseRequestError {
            msg: String::from("invalid header name")
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

#[test]
fn http_headers_parse() {
    let header_content = "header-one: value-one\r\n\r\n";
    let stream = &mut BufReader::new(header_content.as_bytes());
    let result = HttpHeaders::read_from(stream).unwrap();
    let res_headers = Vec::from_iter(result.iter());
    assert_eq!(res_headers.len(), 1);
    assert_eq!(
        &res_headers[0],
        &(
            &String::from("header-one"),
            &vec!(String::from("value-one"))
        )
    );

    // Multiple values for the same header.
    let header_content = "header-one: value-one\r\nheader-one: value-two\r\n\r\n";
    let stream = &mut BufReader::new(header_content.as_bytes());
    let result = HttpHeaders::read_from(stream).unwrap();
    let res_headers = Vec::from_iter(result.iter());
    assert_eq!(res_headers.len(), 1);
    assert_eq!(
        &res_headers[0],
        &(
            &String::from("header-one"),
            &vec!(String::from("value-one"), String::from("value-two"))
        )
    );
}
