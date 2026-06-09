//! Stable opaque identifiers: type-prefixed, collision-resistant, and never
//! derived from a name or path, so renaming or moving never changes an id.

use nanoid::nanoid;

/// Generate a new opaque id like `req_V1StGXR8Z5`. The prefix marks the entity
/// type (`req`, `folder`, `col`, `env`, `ex`).
pub fn new_id(prefix: &str) -> String {
    format!("{}_{}", prefix, nanoid!(11))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_prefixed_and_unique() {
        let a = new_id("req");
        let b = new_id("req");
        assert!(a.starts_with("req_"));
        assert!(a.len() > 4);
        assert_ne!(a, b);
    }
}
