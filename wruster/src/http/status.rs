use std::convert::From;
use std::fmt;

#[allow(missing_docs)]

/// Contains a variant for each defined status code according to
/// the spec in: https://datatracker.ietf.org/doc/html/rfc7231#section-6.1.
#[derive(Debug, PartialEq, Eq)]
pub enum StatusCode {
    // https://datatracker.ietf.org/doc/html/rfc7231#section-6.1
    Continue,
    SwitchingProtocols,
    OK,
    Created,
    Accepted,
    NonAuthoritativeInformation,
    NoContent,
    ResetContent,
    PartialContent,
    MultipleChoices,
    MovedPermanently,
    Found,
    SeeOther,
    NotModified,
    UseProxy,
    TemporaryRedirect,
    BadRequest,
    Unauthorized,
    PaymentRequired,
    Forbidden,
    NotFound,
    MethodNotAllowed,
    NotAcceptable,
    ProxyAuthenticationRequired,
    RequestTimeOut,
    Conflict,
    Gone,
    LengthRequired,
    PreconditionFailed,
    RequestEntityTooLarge,
    RequestURITooLarge,
    UnsupportedMediaType,
    RequestedRangeNotSatisfiable,
    ExpectationFailed,
    InternalServerError,
    NotImplemented,
    BadGateway,
    ServiceUnavailable,
    GatewayTimeOut,
    HTTPVersionNotSupported,
    ExtensionCode(usize),
}

impl StatusCode {
    /**
    Given a status code returns a string containing the reason associated to it.

    # Examples

    ```
    use wruster::http::status::StatusCode;

    let status_code = StatusCode::OK;
    assert_eq!(status_code.reason(), "OK");
    ```
    */
    pub fn reason(&self) -> &'static str {
        match self {
            StatusCode::Continue => "Continue",
            StatusCode::SwitchingProtocols => "Switching Protocols",
            StatusCode::OK => "OK",
            StatusCode::Created => "Created",
            StatusCode::Accepted => "Accepted",
            StatusCode::NonAuthoritativeInformation => "Non-Authoritative Information",
            StatusCode::NoContent => "No Content",
            StatusCode::ResetContent => "Reset Content",
            StatusCode::PartialContent => "Partial Content",
            StatusCode::MultipleChoices => "Multiple Choices",
            StatusCode::MovedPermanently => "Moved Permanently",
            StatusCode::Found => "Found",
            StatusCode::SeeOther => "See Other",
            StatusCode::NotModified => "Not Modified",
            StatusCode::UseProxy => "Use Proxy",
            StatusCode::TemporaryRedirect => "Temporary Redirect",
            StatusCode::BadRequest => "Bad Request",
            StatusCode::Unauthorized => "Unauthorized",
            StatusCode::PaymentRequired => "Payment Required",
            StatusCode::Forbidden => "Forbidden",
            StatusCode::NotFound => "Not Found",
            StatusCode::MethodNotAllowed => "Method Not Allowed",
            StatusCode::NotAcceptable => "Not Acceptable",
            StatusCode::ProxyAuthenticationRequired => "Proxy Authentication Required",
            StatusCode::RequestTimeOut => "Request Time-out",
            StatusCode::Conflict => "Conflict",
            StatusCode::Gone => "Gone",
            StatusCode::LengthRequired => "Length Required",
            StatusCode::PreconditionFailed => "Precondition Failed",
            StatusCode::RequestEntityTooLarge => "Request Entity Too Large",
            StatusCode::RequestURITooLarge => "Request-URI Too Large",
            StatusCode::UnsupportedMediaType => "Unsupported Media Type",
            StatusCode::RequestedRangeNotSatisfiable => "Requested range not satisfiable",
            StatusCode::ExpectationFailed => "Expectation Failed",
            StatusCode::InternalServerError => "Internal Server Error",
            StatusCode::NotImplemented => "Not Implemented",
            StatusCode::BadGateway => "Bad Gateway",
            StatusCode::ServiceUnavailable => "Service Unavailable",
            StatusCode::GatewayTimeOut => "Gateway Time-out",
            StatusCode::HTTPVersionNotSupported => "HTTP Version not supported",
            StatusCode::ExtensionCode(_) => "Extesion code",
        }
    }
}

impl From<usize> for StatusCode {
    fn from(code: usize) -> Self {
        match code {
            100 => Self::Continue,
            101 => Self::SwitchingProtocols,
            200 => Self::OK,
            201 => Self::Created,
            202 => Self::Accepted,
            203 => Self::NonAuthoritativeInformation,
            204 => Self::NoContent,
            205 => Self::ResetContent,
            206 => Self::PartialContent,
            300 => Self::MultipleChoices,
            301 => Self::MovedPermanently,
            302 => Self::Found,
            303 => Self::SeeOther,
            304 => Self::NotModified,
            305 => Self::UseProxy,
            307 => Self::TemporaryRedirect,
            400 => Self::BadRequest,
            401 => Self::Unauthorized,
            402 => Self::PaymentRequired,
            403 => Self::Forbidden,
            404 => Self::NotFound,
            405 => Self::MethodNotAllowed,
            406 => Self::NotAcceptable,
            407 => Self::ProxyAuthenticationRequired,
            408 => Self::RequestTimeOut,
            409 => Self::Conflict,
            410 => Self::Gone,
            411 => Self::LengthRequired,
            412 => Self::PreconditionFailed,
            413 => Self::RequestEntityTooLarge,
            414 => Self::RequestURITooLarge,
            415 => Self::UnsupportedMediaType,
            416 => Self::RequestedRangeNotSatisfiable,
            417 => Self::ExpectationFailed,
            500 => Self::InternalServerError,
            501 => Self::NotImplemented,
            502 => Self::BadGateway,
            503 => Self::ServiceUnavailable,
            504 => Self::GatewayTimeOut,
            505 => Self::HTTPVersionNotSupported,
            code => Self::ExtensionCode(code),
        }
    }
}

impl From<&StatusCode> for usize {
    fn from(code: &StatusCode) -> Self {
        match code {
            StatusCode::Continue => 100,
            StatusCode::SwitchingProtocols => 101,
            StatusCode::OK => 200,
            StatusCode::Created => 201,
            StatusCode::Accepted => 202,
            StatusCode::NonAuthoritativeInformation => 203,
            StatusCode::NoContent => 204,
            StatusCode::ResetContent => 205,
            StatusCode::PartialContent => 206,
            StatusCode::MultipleChoices => 300,
            StatusCode::MovedPermanently => 301,
            StatusCode::Found => 302,
            StatusCode::SeeOther => 303,
            StatusCode::NotModified => 304,
            StatusCode::UseProxy => 305,
            StatusCode::TemporaryRedirect => 307,
            StatusCode::BadRequest => 400,
            StatusCode::Unauthorized => 401,
            StatusCode::PaymentRequired => 402,
            StatusCode::Forbidden => 403,
            StatusCode::NotFound => 404,
            StatusCode::MethodNotAllowed => 405,
            StatusCode::NotAcceptable => 406,
            StatusCode::ProxyAuthenticationRequired => 407,
            StatusCode::RequestTimeOut => 408,
            StatusCode::Conflict => 409,
            StatusCode::Gone => 410,
            StatusCode::LengthRequired => 411,
            StatusCode::PreconditionFailed => 412,
            StatusCode::RequestEntityTooLarge => 413,
            StatusCode::RequestURITooLarge => 414,
            StatusCode::UnsupportedMediaType => 415,
            StatusCode::RequestedRangeNotSatisfiable => 416,
            StatusCode::ExpectationFailed => 417,
            StatusCode::InternalServerError => 500,
            StatusCode::NotImplemented => 501,
            StatusCode::BadGateway => 502,
            StatusCode::ServiceUnavailable => 503,
            StatusCode::GatewayTimeOut => 504,
            StatusCode::HTTPVersionNotSupported => 505,
            StatusCode::ExtensionCode(code) => code.to_owned(),
        }
    }
}

impl fmt::Display for StatusCode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let code: usize = self.into();
        // Status-Code SP Reason-Phrase https://www.w3.org/Protocols/rfc2616/rfc2616-sec6.html
        write!(f, "{} {}", code, self.reason())
    }
}

impl From<StatusCode> for String {
    fn from(code: StatusCode) -> Self {
        code.to_string()
    }
}

impl Clone for StatusCode {
    fn clone(&self) -> Self {
        match self {
            Self::Continue => Self::Continue,
            Self::SwitchingProtocols => Self::SwitchingProtocols,
            Self::OK => Self::OK,
            Self::Created => Self::Created,
            Self::Accepted => Self::Accepted,
            Self::NonAuthoritativeInformation => Self::NonAuthoritativeInformation,
            Self::NoContent => Self::NoContent,
            Self::ResetContent => Self::ResetContent,
            Self::PartialContent => Self::PartialContent,
            Self::MultipleChoices => Self::MultipleChoices,
            Self::MovedPermanently => Self::MovedPermanently,
            Self::Found => Self::Found,
            Self::SeeOther => Self::SeeOther,
            Self::NotModified => Self::NotModified,
            Self::UseProxy => Self::UseProxy,
            Self::TemporaryRedirect => Self::TemporaryRedirect,
            Self::BadRequest => Self::BadRequest,
            Self::Unauthorized => Self::Unauthorized,
            Self::PaymentRequired => Self::PaymentRequired,
            Self::Forbidden => Self::Forbidden,
            Self::NotFound => Self::NotFound,
            Self::MethodNotAllowed => Self::MethodNotAllowed,
            Self::NotAcceptable => Self::NotAcceptable,
            Self::ProxyAuthenticationRequired => Self::ProxyAuthenticationRequired,
            Self::RequestTimeOut => Self::RequestTimeOut,
            Self::Conflict => Self::Conflict,
            Self::Gone => Self::Gone,
            Self::LengthRequired => Self::LengthRequired,
            Self::PreconditionFailed => Self::PreconditionFailed,
            Self::RequestEntityTooLarge => Self::RequestEntityTooLarge,
            Self::RequestURITooLarge => Self::RequestURITooLarge,
            Self::UnsupportedMediaType => Self::UnsupportedMediaType,
            Self::RequestedRangeNotSatisfiable => Self::RequestedRangeNotSatisfiable,
            Self::ExpectationFailed => Self::ExpectationFailed,
            Self::InternalServerError => Self::InternalServerError,
            Self::NotImplemented => Self::NotImplemented,
            Self::BadGateway => Self::BadGateway,
            Self::ServiceUnavailable => Self::ServiceUnavailable,
            Self::GatewayTimeOut => Self::GatewayTimeOut,
            Self::HTTPVersionNotSupported => Self::HTTPVersionNotSupported,
            Self::ExtensionCode(code) => Self::ExtensionCode(*code),
        }
    }
}
