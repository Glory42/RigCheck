//! The only crate allowed to make network requests. Every request goes to one of
//! the three hosts in ROADMAP §6; `deny.toml` keeps HTTP clients out of other crates.

/// Placeholder so the crate has something to test until real code lands.
pub fn name() -> &'static str {
    env!("CARGO_PKG_NAME")
}

#[cfg(test)]
mod tests {
    #[test]
    fn name_matches_package() {
        assert_eq!(super::name(), "rigcheck-net");
    }
}
