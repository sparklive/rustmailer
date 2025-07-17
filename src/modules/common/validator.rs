use std::{
    fmt::{self, Display, Formatter},
    str::FromStr,
};

use email_address::EmailAddress;
use poem_openapi::Validator;

pub struct EmailValidator;

impl Display for EmailValidator {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str("Not a valid email address")
    }
}

impl Validator<String> for EmailValidator {
    fn check(&self, value: &String) -> bool {
        match EmailAddress::from_str(value) {
            Ok(e) => &e.email() == value,
            Err(_) => false,
        }
    }
}
