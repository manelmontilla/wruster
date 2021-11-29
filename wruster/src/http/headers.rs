use std::collections::HashMap;
use std::fmt;
use std::fmt::Debug;
use std::io;
use std::io::prelude::*;

use super::errors::ParseRequestError::Unknow;
use super::errors::*;
use super::HttpMessageChar;
use super::ServerResult;

#[derive(Debug)]
pub struct HttpHeaders {
    headers: HashMap<String, Vec<String>>,
}

impl HttpHeaders {
    pub fn new() -> HttpHeaders {
        HttpHeaders {
            headers: HashMap::new(),
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &Vec<String>)> {
        self.headers.iter()
    }

    pub fn read_from<T: io::Read>(
        from: &mut io::BufReader<T>,
    ) -> Result<HttpHeaders, ParseRequestError> {
        let mut headers = Self::new();
        // https://www.w3.org/Protocols/rfc2616/rfc2616-sec4.html#sec4.5:
        // generic-message = start-line
        //                   *(message-header CRLF)
        //                   CRLF
        //                   [ message-body ]
        debug!("parsing headers");
        loop {
            let header = HttpHeader::read_from(from)?;
            match header {
                None => {
                    break;
                }
                Some(header) => {
                    headers.add_header(header);
                }
            };
        }
        debug!("headers parsed");
        Ok(headers)
    }

    pub fn add_header(&mut self, header: HttpHeader) {
        let name = header.name;
        let content = header.value;
        let values = self.headers.entry(name).or_insert_with(Vec::new);
        values.push(content);
    }

    pub fn get(&self, name: &str) -> Option<&Vec<String>> {
        self.headers.get(name)
    }

    pub fn write<T: io::Write>(&self, to: &mut T) -> ServerResult {
        // https://www.w3.org/Protocols/rfc2616/rfc2616-sec4.html#sec4.5:
        // generic-message = start-line
        //                   *(message-header CRLF)
        //                   CRLF
        //                   [ message-body ]
        for h in self.iter() {
            let name = h.0.clone();
            for content in h.1.iter() {
                let content = content.clone();
                let name = name.clone();
                let header = HttpHeader {
                    name,
                    value: content,
                };
                header.write(to)?
            }
        }

        let written = to.write_all("\r\n".as_bytes());
        if let Err(err) = written {
            return Err(Box::new(err));
        };
        Ok(())
    }
}

impl Default for HttpHeaders {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
pub struct HttpHeader {
    pub name: String,
    pub value: String,
}

impl HttpHeader {
    pub fn read_from<T: io::Read>(
        from: &mut io::BufReader<T>,
    ) -> Result<Option<HttpHeader>, ParseRequestError> {
        //generic-message = start-line
        //                  *(message-header CRLF)
        //                   CRLF
        // Line folding is not supported as specified in:
        // https://www.rfc-editor.org/rfc/rfc7230#section-3.2.4
        let mut line = Vec::<u8>::new();
        loop {
            let mut header_chunk = Vec::<u8>::new();
            if let Err(err) = from.read_until(b'\n', &mut header_chunk) {
                return Err(Unknow(err.to_string()));
            };
            line.append(&mut header_chunk);
            debug!("header chunk read: {}", String::from_utf8_lossy(&line));
            let len = line.len();
            if len < 2 {
                continue;
            }
            if line[len - 1] == b'\n' && line[len - 2] == b'\r' {
                break;
            }
        }
        HttpHeader::parse_header_line(line)
    }

    fn parse_header_line(line: Vec<u8>) -> Result<Option<HttpHeader>, ParseRequestError> {
        // header-field   = field-name ":" OWS field-value OWS
        // field-name     = token
        // field-value    = *( field-content / obs-fold )
        // field-content  = field-vchar [ 1*( SP / HTAB ) field-vchar ]
        // field-vchar    = VCHAR / obs-text
        assert!(line.len() >= 2);
        if line.len() == 2 {
            return Ok(None);
        };
        debug!("header line {}", String::from_utf8_lossy(&line));
        // Remove the \r\n at the end of the header line.
        let line = &line[..line.len() - 2];
        // Read the field-name which is a token:
        // https://www.rfc-editor.org/rfc/rfc7230#section-3.2.6
        let mut name = String::new();
        let mut i = 0;
        while i < line.len() {
            let c = line[i] as char;
            if !c.is_valid_token_char() {
                break;
            };
            name.push(c);
            i += 1;
        }
        if name.is_empty() {
            debug!(
                "invalid header name line: {}, missing header name",
                String::from_utf8_lossy(line)
            );
            return Err(Unknow(String::from("invalid header name line")));
        };
        // After the token we MUST receive a colon.
        if line[i] != b':' {
            error!(
                "invalid header line: {}, missing semicolon",
                String::from_utf8_lossy(line)
            );
            return Err(Unknow(String::from("invalid header name line")));
        };
        // The header value must have at least one octed.
        let mut header_value_start = i + 1;
        let header_value_length = line.len() - header_value_start;
        if header_value_length < 1 {
            debug!(
                "invalid header value line: {}",
                String::from_utf8_lossy(line)
            );
            return Err(Unknow(String::from("invalid header name value")));
        };
        // We don't support folding so the field-value = field-content.
        let mut field_value = String::new();
        // Skip the initial optional white space if any.
        let mut j = header_value_start;
        while j < line.len() && (line[j] as char).is_optional_white_space() {
            j += 1;
        }
        header_value_start = j;
        for j in header_value_start..line.len() {
            let c = line[j] as char;
            if !c.is_valid_field_content() {
                debug!(
                    "invalid header value line: {}, char {}, position {}",
                    String::from_utf8_lossy(line),
                    c,
                    j
                );
                return Err(Unknow(String::from("invalid header name value")));
            }
            field_value.push(c);
        }

        let header = HttpHeader {
            name,
            value: field_value,
        };
        debug!("header parsed: {}", header);
        Ok(Some(header))
    }

    pub fn write<T: io::Write>(&self, to: &mut T) -> ServerResult {
        // https://www.w3.org/Protocols/rfc2616/rfc2616-sec4.html#sec4.5:
        // generic-message = start-line
        //                   *(message-header CRLF)
        //                   CRLF
        //                   [ message-body ]
        // header-field   = field-name ":" OWS field-value OWS
        // field-name     = token
        let mut written = to.write_all(&self.name.as_bytes());
        if let Err(err) = written {
            return Err(Box::new(err));
        };
        written = to.write_all(": ".as_bytes());
        if let Err(err) = written {
            return Err(Box::new(err));
        };
        written = to.write_all(self.value.as_bytes());
        if let Err(err) = written {
            return Err(Box::new(err));
        };
        written = to.write_all("\r\n".as_bytes());
        if let Err(err) = written {
            return Err(Box::new(err));
        };
        Ok(())
    }
}

impl fmt::Display for HttpHeader {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}:{}", self.name, self.value)
    }
}
