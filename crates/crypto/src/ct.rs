//! Constant-time helpers for secret / MAC comparisons.

use subtle::ConstantTimeEq;

/// Constant-time equality for equal-length byte slices.
/// Returns `false` immediately only when lengths differ (length is not secret
/// for hex digests of fixed algorithms); content compare is always CT.
pub fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    bool::from(a.ct_eq(b))
}

/// Constant-time equality for UTF-8 strings (e.g. hex digests).
pub fn ct_eq_str(a: &str, b: &str) -> bool {
    ct_eq(a.as_bytes(), b.as_bytes())
}
