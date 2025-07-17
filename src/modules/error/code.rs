use poem::http::StatusCode;
use poem_openapi::Enum;

#[derive(Copy, Clone, Debug, Enum, Eq, PartialEq)]
#[repr(u32)]
pub enum ErrorCode {
    InvalidParameter = 10000,
    NetworkError = 10010,
    ConnectionTimeout = 10020,
    ConnectionPoolTimeout = 10030,
    InternalError = 10040,
    ResourceNotFound = 10050,
    AccountDisabled = 10060,
    AutoconfigFetchFailed = 10070,
    LicenseAccountLimitReached = 10080,
    LicenseExpired = 10090,
    InvalidLicense = 10100,
    ImapCommandFailed = 10110,
    ImapAuthenticationFailed = 10120,
    ImapUnexpectedResult = 10130,
    PermissionDenied = 10140,
    HttpResponseError = 10150,
    NatsRequestFailed = 10160,
    NatsConnectionFailed = 10170,
    NatsCreateStreamFailed = 10180,
    VRLScriptSyntaxError = 10190,
    AlreadyExists = 10200,
    MissingConfiguration = 10210,
    Incompatible = 10220,
    MailBoxNotCached = 10230,
    ExceedsLimitation = 10240,
    OAuth2ItemDisabled = 10250,
    MissingRefreshToken = 10260,
    EmlFileParseError = 10270,
    SmtpCommandFailed = 10280,
    SmtpConnectionFailed = 10290,
    TooManyRequest = 10300,
    MissingContentLength = 10310,
    PayloadTooLarge = 10320,
    RequestTimeout = 10330,
    MethodNotAllowed = 10340,
    UnhandledPoemError = 10350,
}

impl ErrorCode {
    pub fn status(&self) -> StatusCode {
        match self {
            ErrorCode::InvalidParameter
            | ErrorCode::VRLScriptSyntaxError
            | ErrorCode::MissingConfiguration
            | ErrorCode::Incompatible
            | ErrorCode::ExceedsLimitation
            | ErrorCode::EmlFileParseError => StatusCode::BAD_REQUEST,
            ErrorCode::PermissionDenied => StatusCode::UNAUTHORIZED,
            ErrorCode::AccountDisabled
            | ErrorCode::LicenseAccountLimitReached
            | ErrorCode::LicenseExpired
            | ErrorCode::InvalidLicense
            | ErrorCode::OAuth2ItemDisabled => StatusCode::FORBIDDEN,
            ErrorCode::ResourceNotFound => StatusCode::NOT_FOUND,
            ErrorCode::RequestTimeout => StatusCode::REQUEST_TIMEOUT,
            ErrorCode::AlreadyExists => StatusCode::CONFLICT,
            ErrorCode::MissingContentLength => StatusCode::LENGTH_REQUIRED,
            ErrorCode::PayloadTooLarge => StatusCode::PAYLOAD_TOO_LARGE,
            ErrorCode::TooManyRequest => StatusCode::TOO_MANY_REQUESTS,
            ErrorCode::InternalError
            | ErrorCode::AutoconfigFetchFailed
            | ErrorCode::ImapCommandFailed
            | ErrorCode::ImapUnexpectedResult
            | ErrorCode::HttpResponseError
            | ErrorCode::NatsRequestFailed
            | ErrorCode::NatsCreateStreamFailed
            | ErrorCode::MailBoxNotCached
            | ErrorCode::ImapAuthenticationFailed
            | ErrorCode::MissingRefreshToken
            | ErrorCode::SmtpCommandFailed
            | ErrorCode::NetworkError
            | ErrorCode::ConnectionTimeout
            | ErrorCode::ConnectionPoolTimeout
            | ErrorCode::NatsConnectionFailed
            | ErrorCode::UnhandledPoemError
            | ErrorCode::SmtpConnectionFailed => StatusCode::INTERNAL_SERVER_ERROR,
            ErrorCode::MethodNotAllowed => StatusCode::METHOD_NOT_ALLOWED,
        }
    }
}