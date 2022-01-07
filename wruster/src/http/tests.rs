use super::*;
use std::io::BufReader;
use std::iter::FromIterator;

#[test]
fn http_header_parse_standard() {
    // Test parsing standard headers.
    let header_content = "Header-Name:header value\r\n";
    let mut stream = BufReader::new(header_content.as_bytes());
    let header = Header::read_from(&mut stream).unwrap().unwrap();
    assert_eq!(header.name.as_str(), "Header-Name");
    assert_eq!(header.value.as_str(), "header value");
}

#[test]
fn http_header_parse_normalize() {
    let header_content = "header:header value\r\n";
    let mut stream = BufReader::new(header_content.as_bytes());
    let header = Header::read_from(&mut stream).unwrap().unwrap();
    assert_eq!(header.name.as_str(), "Header");

    let header_content = "header-name:header value\r\n";
    let mut stream = BufReader::new(header_content.as_bytes());
    let header = Header::read_from(&mut stream).unwrap().unwrap();
    assert_eq!(header.name.as_str(), "Header-Name");

    let header_content = "header-name-1:header value\r\n";
    let mut stream = BufReader::new(header_content.as_bytes());
    let header = Header::read_from(&mut stream).unwrap().unwrap();
    assert_eq!(header.name.as_str(), "Header-Name-1");

    let header_content = "header_part1-part2-part3: header value\r\n";
    let mut stream = BufReader::new(header_content.as_bytes());
    let header = Header::read_from(&mut stream).unwrap().unwrap();
    assert_eq!(header.name.as_str(), "Header_part1-Part2-Part3");
}

#[test]
fn http_header_parse_with_colon_values() {
    let header_content = "Host: localhost:1234\r\n";
    let mut stream = BufReader::new(header_content.as_bytes());
    let header_content = Header::read_from(&mut stream).unwrap().unwrap();
    assert_eq!(header_content.name.as_str(), "Host");
    assert_eq!(header_content.value.as_str(), "localhost:1234");
}

#[test]
fn http_header_invalid_tokens() {
    // Only accepts valid chars in tokens.
    let header_content = "header\rname:headervalue\r\n";
    let mut stream = BufReader::new(header_content.as_bytes());
    assert_eq!(
        Header::read_from(&mut stream).unwrap_err(),
        Unknown(String::from("invalid header name line"))
    );

    let header_content = "header\x0Bname:headervalue\r\n";
    let mut stream = BufReader::new(header_content.as_bytes());
    assert_eq!(
        Header::read_from(&mut stream).unwrap_err(),
        Unknown(String::from("invalid header name line"))
    );
}

#[test]
fn http_header_no_colon() {
    let header_content = "header-name :headervalue\r\n";
    let mut stream = BufReader::new(header_content.as_bytes());
    assert_eq!(
        Header::read_from(&mut stream).unwrap_err(),
        Unknown(String::from("invalid header name line"))
    );
}

#[test]
fn http_header_invalid_header_values() {
    let header_content = "header-name: \0x1Aheadervalue\r\n";
    let mut stream = BufReader::new(header_content.as_bytes());
    assert_eq!(
        Header::read_from(&mut stream).unwrap_err(),
        Unknown(String::from("invalid header name value"))
    );
}

#[test]
fn http_headers_parse() {
    let header_content = "header-one: value-one\r\n\r\n";
    let stream = &mut BufReader::new(header_content.as_bytes());
    let result = Headers::read_from(stream).unwrap();
    let res_headers = Vec::from_iter(result.iter());
    assert_eq!(res_headers.len(), 1);
    assert_eq!(
        &res_headers[0],
        &(
            &String::from("Header-One"),
            &vec!(String::from("value-one"))
        )
    );

    // Multiple values for the same header.
    let header_content = "header-one: value-one\r\nheader-one: value-two\r\n\r\n";
    let stream = &mut BufReader::new(header_content.as_bytes());
    let result = Headers::read_from(stream).unwrap();
    let res_headers = Vec::from_iter(result.iter());
    assert_eq!(res_headers.len(), 1);
    assert_eq!(
        &res_headers[0],
        &(
            &String::from("Header-One"),
            &vec!(String::from("value-one"), String::from("value-two"))
        )
    );
}

#[test]
fn http_request_from_str() {
    let str_req = "POST /file HTTP/1.1\r\n\
Content-Length: 4\r\n\
\r\n\
test";
    let req = Request::read_from_str(str_req).unwrap();
    let mut body = req.body.unwrap();
    let mut payload = String::new();
    body.content.read_to_string(&mut payload).unwrap();

    assert_eq!(req.uri, "/file");
    assert_eq!(req.method, HttpMethod::POST);
    assert_eq!(&payload, "test");
}

#[test]
fn http_body_write() {
    let content = "#wruster";
    let mut body = Body {
        content: Box::new(Cursor::new(content)),
        content_type: Some(mime::TEXT_PLAIN),
        content_length: content.len() as u64,
    };
    let mut to: Vec<u8> = Vec::new();

    body.write(&mut to).unwrap();
    let got_content = String::from_utf8(to).unwrap();
    assert_eq!(content, &got_content)
}

#[test]
fn http_response_write() {
    let content = "#wruster";
    let body = Body {
        content: Box::new(Cursor::new(content)),
        content_type: Some(mime::TEXT_PLAIN),
        content_length: content.len() as u64,
    };

    let mut headers = Headers::new();
    headers.add(Header {
        name: String::from("Content-Length"),
        value: String::from("8"),
    });
    let mut response = Response {
        status: StatusCode::OK,
        headers: headers,
        body: Some(body),
    };

    let mut to: Vec<u8> = Vec::new();
    response.write(&mut to).unwrap();

    let got = String::from_utf8(to).unwrap();
    let want = "HTTP/1.1 200 OK\r\nContent-Length: 8\r\n\r\n#wruster";
    assert_eq!(want, &got)
}

#[test]
fn http_response_write_empty_body() {
    let headers = Headers::new();
    let mut response = Response {
        status: StatusCode::OK,
        headers: headers,
        body: None,
    };

    let mut to: Vec<u8> = Vec::new();
    response.write(&mut to).unwrap();

    let got = String::from_utf8(to).unwrap();
    let want = "HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n";
    assert_eq!(want, &got)
}

#[test]
fn http_response_no_headers_no_body() {
    let headers = Headers::new();
    let mut response = Response {
        status: StatusCode::OK,
        headers: headers,
        body: None,
    };
    let mut to: Vec<u8> = Vec::new();
    response.write(&mut to).unwrap();
    let got = String::from_utf8(to).unwrap();
    let want = "HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n";
    assert_eq!(want, &got)
}

#[test]
fn http_body_read_from_invalid_content_type() {
    let from = Cursor::new("test");
    let mut headers = Headers::new();
    headers.add(Header {
        name: "Content-Type".to_string(),
        value: "invalid".to_string(),
    });
    headers.add(Header {
        name: "Content-Length".to_string(),
        value: "4".to_string(),
    });
    assert!(Body::read_from(from, &headers).is_err(), "");
}
