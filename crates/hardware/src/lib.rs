//! Linux hardware detection (CPU, GPU, RAM, VRAM, Vulkan, driver, disk).

/// Placeholder so the crate has something to test until real code lands.
pub fn name() -> &'static str {
    env!("CARGO_PKG_NAME")
}

#[cfg(test)]
mod tests {
    #[test]
    fn name_matches_package() {
        assert_eq!(super::name(), "rigcheck-hardware");
    }
}
