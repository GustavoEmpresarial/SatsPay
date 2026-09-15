use super::*;
use std::net::{Ipv4Addr, Ipv6Addr};

fn ipv4(a: u8, b: u8, c: u8, d: u8) -> IpAddr {
    IpAddr::V4(Ipv4Addr::new(a, b, c, d))
}

#[test]
fn blocks_loopback_private_and_metadata_addresses() {
    assert!(is_blocked_ip(ipv4(127, 0, 0, 1)));
    assert!(is_blocked_ip(ipv4(10, 0, 0, 5)));
    assert!(is_blocked_ip(ipv4(192, 168, 1, 10)));
    assert!(is_blocked_ip(ipv4(172, 16, 4, 4)));
    // Cloud metadata — the classic SSRF credential-theft target.
    assert!(is_blocked_ip(ipv4(169, 254, 169, 254)));
    assert!(is_blocked_ip(ipv4(100, 64, 0, 1)));
    assert!(is_blocked_ip(ipv4(0, 0, 0, 0)));
    assert!(is_blocked_ip(IpAddr::V6(Ipv6Addr::LOCALHOST)));
    assert!(is_blocked_ip("fd00::1".parse().unwrap()));
    assert!(is_blocked_ip("fe80::1".parse().unwrap()));
    // IPv4-mapped loopback must not slip past the v6 arm.
    assert!(is_blocked_ip("::ffff:127.0.0.1".parse().unwrap()));
}

#[test]
fn allows_public_addresses() {
    assert!(!is_blocked_ip(ipv4(8, 8, 8, 8)));
    assert!(!is_blocked_ip(ipv4(1, 1, 1, 1)));
    assert!(!is_blocked_ip(ipv4(203, 0, 114, 10)));
    assert!(!is_blocked_ip("2606:4700::1111".parse().unwrap()));
}

#[test]
fn blocks_documentation_ranges() {
    // TEST-NET-1/2/3 are not routable; a callback pointed there is a mistake
    // or a probe, never a real merchant endpoint.
    assert!(is_blocked_ip(ipv4(203, 0, 113, 10)));
    assert!(is_blocked_ip(ipv4(198, 51, 100, 7)));
    assert!(is_blocked_ip(ipv4(192, 0, 2, 1)));
}

#[test]
fn callback_url_requires_https_and_public_host() {
    assert!(validate_callback_url("https://merchant.example/hook").is_ok());
    assert_eq!(validate_callback_url("http://merchant.example/hook"), Err(CallbackUrlError::NotHttps));
    assert_eq!(validate_callback_url("ftp://merchant.example/hook"), Err(CallbackUrlError::NotHttps));
    assert_eq!(validate_callback_url("not a url"), Err(CallbackUrlError::NotAUrl));
    assert_eq!(validate_callback_url("https://localhost/hook"), Err(CallbackUrlError::PrivateHost));
    assert_eq!(validate_callback_url("https://app.localhost/hook"), Err(CallbackUrlError::PrivateHost));
    assert_eq!(validate_callback_url("https://127.0.0.1/hook"), Err(CallbackUrlError::PrivateHost));
    assert_eq!(validate_callback_url("https://169.254.169.254/latest/meta-data/"), Err(CallbackUrlError::PrivateHost));
    assert_eq!(validate_callback_url("https://[::1]/hook"), Err(CallbackUrlError::PrivateHost));
}

#[test]
fn retry_backoff_grows_and_is_capped() {
    assert_eq!(next_retry_delay(0), Duration::from_secs(30));
    assert_eq!(next_retry_delay(1), Duration::from_secs(60));
    assert_eq!(next_retry_delay(2), Duration::from_secs(120));
    assert_eq!(next_retry_delay(3), Duration::from_secs(240));
    // Ceiling holds for every later attempt.
    assert_eq!(next_retry_delay(MAX_WEBHOOK_ATTEMPTS), Duration::from_secs(3600));
    assert_eq!(next_retry_delay(99), Duration::from_secs(3600));
}
