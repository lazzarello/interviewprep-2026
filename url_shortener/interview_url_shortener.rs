//! In-memory port of termie's first URL shortener example.
//! shortcut: links are lost on exit, add persistence when links must survive restarts.

use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const BASE62_ALPHABET: &[u8; 62] =
    b"0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";

fn encode_base62(mut num: u64) -> String {
    let mut chars = Vec::new();
    loop {
        chars.push(BASE62_ALPHABET[(num % 62) as usize] as char);
        num /= 62;
        if num == 0 {
            break;
        }
    }
    chars.into_iter().rev().collect()
}

fn decode_base62(code: &str) -> Result<u64, &'static str> {
    code.bytes().try_fold(0_u64, |num, character| {
        let digit = BASE62_ALPHABET
            .iter()
            .position(|&c| c == character)
            .ok_or("Invalid Base62 character")?;
        num.checked_mul(62)
            .and_then(|n| n.checked_add(digit as u64))
            .ok_or("Base62 value exceeds u64")
    })
}

fn now_seconds() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("System clock is before the Unix epoch")
        .as_secs_f64()
}

struct ShortUrl {
    original_url: String,
    expires_at: Option<f64>,
}

impl ShortUrl {
    fn is_expired(&self) -> bool {
        self.expires_at.is_some_and(|expiry| now_seconds() > expiry)
    }
}

struct UrlShortener {
    base_url: String,
    counter: u64,
    code_to_url: HashMap<String, ShortUrl>,
    url_to_code: HashMap<String, String>,
}

impl UrlShortener {
    fn new(base_url: &str) -> Self {
        Self {
            base_url: base_url.to_owned(),
            counter: 100_000,
            code_to_url: HashMap::new(),
            url_to_code: HashMap::new(),
        }
    }

    fn shorten(
        &mut self,
        original_url: &str,
        custom_alias: Option<&str>,
        ttl_seconds: Option<f64>,
    ) -> Result<String, String> {
        if custom_alias.is_none() {
            if let Some(code) = self.url_to_code.get(original_url) {
                if let Some(record) = self.code_to_url.get(code) {
                    if !record.is_expired() {
                        return Ok(format!("{}{code}", self.base_url));
                    }
                }
            }
        }

        let alias = custom_alias.filter(|alias| !alias.is_empty());
        let code = if let Some(alias) = alias {
            if self.code_to_url.contains_key(alias) {
                return Err(format!("Alias '{alias}' already in use."));
            }
            alias.to_owned()
        } else {
            // Custom aliases share the generated-code namespace; never overwrite one.
            loop {
                let candidate = encode_base62(self.counter);
                self.counter = self.counter.checked_add(1).ok_or("ID space exhausted")?;
                if !self.code_to_url.contains_key(&candidate) {
                    break candidate;
                }
            }
        };

        let expires_at = ttl_seconds
            .filter(|&ttl| ttl != 0.0)
            .map(|ttl| now_seconds() + ttl);
        self.code_to_url.insert(
            code.clone(),
            ShortUrl {
                original_url: original_url.to_owned(),
                expires_at,
            },
        );
        if alias.is_none() {
            self.url_to_code
                .insert(original_url.to_owned(), code.clone());
        }
        Ok(format!("{}{code}", self.base_url))
    }

    fn resolve(&mut self, short_code: &str) -> Option<&str> {
        if self.code_to_url.get(short_code)?.is_expired() {
            let record = self.code_to_url.remove(short_code)?;
            // An expired alias or older code must not remove a newer URL mapping.
            if self
                .url_to_code
                .get(&record.original_url)
                .map(String::as_str)
                == Some(short_code)
            {
                self.url_to_code.remove(&record.original_url);
            }
            return None;
        }
        self.code_to_url
            .get(short_code)
            .map(|record| record.original_url.as_str())
    }
}

fn main() -> Result<(), String> {
    let mut service = UrlShortener::new("https://sho.rt/");
    println!("=== Interview-Ready URL Shortener Demo ===");

    let original = "https://example.com/very/long/path?param=123";
    let u1 = service.shorten(original, None, None)?;
    let code1 = u1.rsplit('/').next().unwrap();
    println!("Shortened URL 1: {u1}");
    println!("Resolved code '{code1}' -> {:?}", service.resolve(code1));
    println!("Decoded Base62 ID: {}", decode_base62(code1)?);

    let u1_again = service.shorten(original, None, None)?;
    println!(
        "Idempotent check (same URL): {u1_again} (matches: {})",
        u1 == u1_again
    );

    let u2 = service.shorten("https://github.com/termie", Some("termie-gh"), None)?;
    println!("Custom alias: {u2}");
    println!("Resolved 'termie-gh' -> {:?}", service.resolve("termie-gh"));

    let u3 = service.shorten("https://temp-link.org", None, Some(0.1))?;
    let code3 = u3.rsplit('/').next().unwrap();
    println!("Temporary short code: {code3}");
    println!("Immediately resolved: {:?}", service.resolve(code3));
    std::thread::sleep(Duration::from_millis(150));
    println!(
        "Resolved after TTL expired (0.15s): {:?} (None expected)",
        service.resolve(code3)
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base62_round_trips_and_validates() {
        for (number, code) in [
            (0, "0"),
            (9, "9"),
            (10, "a"),
            (35, "z"),
            (36, "A"),
            (61, "Z"),
            (62, "10"),
            (100_000, "q0U"),
        ] {
            assert_eq!(encode_base62(number), code);
            assert_eq!(decode_base62(code), Ok(number));
        }
        assert_eq!(decode_base62(&encode_base62(u64::MAX)), Ok(u64::MAX));
        assert_eq!(decode_base62(""), Ok(0));
        assert!(decode_base62("not-valid!").is_err());
        assert!(decode_base62("é").is_err());
        assert!(decode_base62("ZZZZZZZZZZZZ").is_err());
    }

    #[test]
    fn shortening_is_idempotent_and_resolves() {
        let mut service = UrlShortener::new("https://test/");
        let short = service.shorten("https://example.com", None, None).unwrap();
        assert_eq!(short, "https://test/q0U");
        assert_eq!(
            service.shorten("https://example.com", None, None),
            Ok(short)
        );
        assert_eq!(service.resolve("q0U"), Some("https://example.com"));
        assert_eq!(service.resolve("missing"), None);
    }

    #[test]
    fn custom_aliases_reject_duplicates_and_do_not_overwrite_generated_codes() {
        let mut service = UrlShortener::new("https://sho.rt/");
        assert_eq!(
            service
                .shorten("https://custom", Some("q0U"), None)
                .unwrap(),
            "https://sho.rt/q0U"
        );
        assert!(service.shorten("https://other", Some("q0U"), None).is_err());
        assert_eq!(
            service.shorten("https://generated", None, None).unwrap(),
            "https://sho.rt/q0V"
        );
        assert_eq!(service.resolve("q0U"), Some("https://custom"));
        assert_eq!(service.resolve("q0V"), Some("https://generated"));
    }

    #[test]
    fn expired_links_are_removed_without_losing_newer_mappings() {
        let mut service = UrlShortener::new("https://sho.rt/");
        service
            .shorten("https://example.com", None, Some(-1.0))
            .unwrap();
        let replacement = service.shorten("https://example.com", None, None).unwrap();
        assert_eq!(service.resolve("q0U"), None);
        assert!(!service.code_to_url.contains_key("q0U"));
        assert_eq!(
            service.shorten("https://example.com", None, None),
            Ok(replacement)
        );
    }

    #[test]
    fn expiring_alias_does_not_remove_the_generated_mapping() {
        let mut service = UrlShortener::new("https://sho.rt/");
        let short = service.shorten("https://example.com", None, None).unwrap();
        service
            .shorten("https://example.com", Some("temporary"), Some(-1.0))
            .unwrap();
        assert_eq!(service.resolve("temporary"), None);
        assert_eq!(
            service.shorten("https://example.com", None, None),
            Ok(short)
        );
    }

    #[test]
    fn zero_ttl_and_empty_alias_match_python_behavior() {
        let mut service = UrlShortener::new("https://sho.rt/");
        service
            .shorten("https://example.com", Some(""), Some(0.0))
            .unwrap();
        assert_eq!(service.resolve("q0U"), Some("https://example.com"));
        assert!(service.code_to_url["q0U"].expires_at.is_none());
        assert_eq!(
            service.shorten("https://example.com", None, None).unwrap(),
            "https://sho.rt/q0U"
        );
    }
}
