//! Primitive di sicurezza senza I/O: hash delle password, token JWT, token
//! opachi, codici OTP, cookie e validazione di username/password/email.
//!
//! Tutto codice sicuro (`unsafe` è vietato a livello di workspace).

pub mod cookie;
pub mod jwt;
pub mod otp;
pub mod password;
pub mod token;
pub mod validate;

/// Confronto a tempo costante di due stringhe (token, codici, CSRF).
pub fn constant_time_eq(a: &str, b: &str) -> bool {
    use subtle::ConstantTimeEq;
    a.as_bytes().ct_eq(b.as_bytes()).into()
}

#[cfg(test)]
mod tests {
    #[test]
    fn constant_time_eq_compares_content_and_length() {
        assert!(super::constant_time_eq("abc", "abc"));
        assert!(!super::constant_time_eq("abc", "abd"));
        assert!(!super::constant_time_eq("abc", "abcd"));
        assert!(super::constant_time_eq("", ""));
    }
}
