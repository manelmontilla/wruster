use std::collections::HashMap;
use std::fmt;
use std::fmt::Debug;
use std::io;
use std::io::prelude::*;

use super::errors::HttpError::InvalidRequest;
use super::errors::*;
use super::HttpResult;
use super::MessageChar;

#[derive(Debug)]
/// Holds a collection of HTTP headers.
pub struct Headers {
    headers: HashMap<String, Vec<String>>,
}

impl Headers {
    /**
    Creates a new [`Headers`] struct.

    # Examples

    ```
    use wruster::http::headers::Headers;

    let headers = Headers::new();

    ```
    */
    pub fn new() -> Headers {
        Headers {
            headers: HashMap::new(),
        }
    }

    /**
    Returns an iterator over the current headers. Note that per each
    header, there could be multiple values.

    # Examples

    ```
    use wruster::http::headers::{Headers, Header};

    let mut headers = Headers::new();
    let header = Header{
       name:String::from("name"),
       value:String::from("value")
    };
    headers.add(header);
    for header in headers.iter() {
       print!("{:?}", header);
    };
    ```
    */
    pub fn iter(&self) -> impl Iterator<Item = (&String, &Vec<String>)> {
        self.headers.iter()
    }

    /**
    Reads the headers from an HTTP message in a [`io::BufReader`] according to
    the spec: <https://datatracker.ietf.org/doc/html/rfc7230>.

    # Errors

    Returns a [`HttpError`] if the header does not conform to the spec: <https://datatracker.ietf.org/doc/html/rfc7230>
    or there is any problem reading from the ``to``parameter.
    */
    pub fn read_from<T: io::Read>(from: &mut io::BufReader<T>) -> Result<Headers, HttpError> {
        let mut headers = Self::new();
        // generic-message = start-line
        //                   *(message-header CRLF)
        //                   CRLF
        //                   [ message-body ]
        debug!("parsing headers");
        loop {
            let header = Header::read_from(from)?;
            match header {
                None => {
                    break;
                }
                Some(header) => {
                    headers.add(header);
                }
            };
        }
        debug!("headers parsed");
        Ok(headers)
    }

    /**
    Adds a header to the the collection.

    # Examples
    ```
    use wruster::http::headers::{Headers, Header};

    let mut headers = Headers::new();
    let header = Header{
       name:String::from("name"),
       value:String::from("value")
    };
    headers.add(header);
    ```
    */
    pub fn add(&mut self, header: Header) {
        let name = header.name;
        let content = header.value;
        let values = self.headers.entry(name).or_insert_with(Vec::new);
        values.push(content);
    }

    /**
    Returns the values of a header given its name.

    # Examples
    ```
    use wruster::http::headers::{Headers, Header};

    let mut headers = Headers::new();
    let name = String::from("name");
    let header = Header{
     name,
    value:String::from("value")
    };
    headers.add(header);
    let value = headers.get("name");
    assert_eq!(
       value,
       Some(
           &vec!(String::from("value"))
        )
    );
    ```
    */
    pub fn get(&self, name: &str) -> Option<&Vec<String>> {
        self.headers.get(name)
    }

    /**
    Writes the headers to a type implementing [``io::Write``]
    according to the spec: <https://datatracker.ietf.org/doc/html/rfc7230>.

     # Examples
    ```
    use wruster::http::headers::{Headers, Header};

    let mut headers = Headers::new();
    let name = String::from("name");
    let header = Header{
     name,
    value:String::from("value")
    };
    headers.add(header);
    let mut to: Vec<u8> = Vec::new();
    headers.write(&mut to).unwrap();
    ```

    # Errors

    This function will return an error if there is any problem
    writing ``to`` parameter.
    */
    pub fn write<T: io::Write>(&self, to: &mut T) -> HttpResult<()> {
        // generic-message = start-line
        //                   *(message-header CRLF)
        //                   CRLF
        //                   [ message-body ]
        for h in self.iter() {
            let name = h.0.clone();
            for content in h.1.iter() {
                let content = content.clone();
                let name = name.clone();
                let header = Header {
                    name,
                    value: content,
                };
                header.write(to)?
            }
        }
        to.write_all("\r\n".as_bytes()).map_err(HttpError::from)?;
        Ok(())
    }
}

impl Default for Headers {
    fn default() -> Self {
        Self::new()
    }
}

/// Represents an HTTP header.
#[derive(Debug)]
pub struct Header {
    /// The name of the header.
    pub name: String,
    /// The value of the header.
    pub value: String,
}

impl Header {
    /**
    Reads an header from an HTTP message in a [`io::BufReader`] according to
    the spec: <https://datatracker.ietf.org/doc/html/rfc7230>.

    # Examples

    ## TODO

    # Errors

    Returns a [`HttpError`] if the header does not conform to the spec:
    <https://datatracker.ietf.org/doc/html/rfc7230>.
    */
    pub fn read_from<T: io::Read>(
        from: &mut io::BufReader<T>,
    ) -> Result<Option<Header>, HttpError> {
        //generic-message = start-line
        //                  *(message-header CRLF)
        //                   CRLF
        // Line folding is not supported as specified in:
        // https://www.rfc-editor.org/rfc/rfc7230#section-3.2.4
        let mut line = Vec::<u8>::new();
        loop {
            let mut header_chunk = Vec::<u8>::new();
            from.read_until(b'\n', &mut header_chunk)
                .map_err(HttpError::from)?;
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
        Header::parse_header_line(line)
    }

    fn parse_header_line(line: Vec<u8>) -> Result<Option<Header>, HttpError> {
        // header-field   = field-name ":" OWS field-value OWS
        // field-name     = token
        // field-value    = *( field-content / obs-fold )
        // field-content  = field-vchar [ 1*( SP / HTAB ) field-vchar ]
        // field-vchar    = VCHAR / obs-text
        // token          = 1*<any CHAR except CTLs or separators>
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
            return Err(InvalidRequest("invalid header name line".to_string()));
        };
        // After the token we MUST receive a colon.
        if line[i] != b':' {
            error!(
                "invalid header line: {}, missing semicolon",
                String::from_utf8_lossy(line)
            );
            return Err(InvalidRequest("invalid header name line".to_string()));
        };
        // The header value must have at least one octed.
        let mut header_value_start = i + 1;
        let header_value_length = line.len() - header_value_start;
        if header_value_length < 1 {
            debug!(
                "invalid header value line: {}",
                String::from_utf8_lossy(line)
            );
            return Err(InvalidRequest("invalid header name value".to_string()));
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
                return Err(InvalidRequest("invalid header name value".to_string()));
            }
            field_value.push(c);
        }
        name = normalize_header_name(name);
        let header = Header {
            name,
            value: field_value,
        };
        debug!("header parsed: {}", header);
        Ok(Some(header))
    }

    /**
    Writes the header to a type implementing the [``io::Write``] trait.

    # Examples

    ```
    use wruster::http::headers::{Headers, Header};

    let mut headers = Headers::new();
    let name = String::from("name");
    let header = Header{
     name,
    value:String::from("value")
    };
    let mut to: Vec<u8> = Vec::new();
    headers.write(&mut to).unwrap();
    ```

    # Errors

    This function will return an error if there is any error writing
    to the ``to`` paramerer.
    */
    pub fn write<T: io::Write>(&self, to: &mut T) -> HttpResult<()> {
        // generic-message = start-line
        //                   *(message-header CRLF)
        //                   CRLF
        //                   [ message-body ]
        // header-field   = field-name ":" OWS field-value OWS
        // field-name     = token
        let mut written = to.write_all(self.name.as_bytes());
        if let Err(err) = written {
            debug!("error writing Header: {:?}", err);
            return Err(HttpError::Unknown(err.to_string()));
        };
        written = to.write_all(": ".as_bytes());
        if let Err(err) = written {
            return Err(HttpError::Unknown(err.to_string()));
        };
        written = to.write_all(self.value.as_bytes());
        if let Err(err) = written {
            return Err(HttpError::Unknown(err.to_string()));
        };
        written = to.write_all("\r\n".as_bytes());
        if let Err(err) = written {
            return Err(HttpError::Unknown(err.to_string()));
        };
        Ok(())
    }
}

impl fmt::Display for Header {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}:{}", self.name, self.value)
    }
}

fn normalize_header_name(name: String) -> String {
    let mut normalized = String::new();
    let mut next_alfanum_upper = true;
    for c in name.chars() {
        if c.is_ascii_alphanumeric() && next_alfanum_upper {
            normalized.push(c.to_ascii_uppercase() as char);
            next_alfanum_upper = false;
            continue;
        };
        if c == char::from(b' ') || c == char::from(b'-') {
            next_alfanum_upper = true;
        }
        normalized.push(c);
    }
    normalized
}
