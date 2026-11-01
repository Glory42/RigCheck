//! Data pipeline: crawl Steam, parse requirements, build games.db.

fn main() {
    println!("{}", banner());
}

fn banner() -> String {
    format!("rigcheck-pipeline {}", env!("CARGO_PKG_VERSION"))
}

#[cfg(test)]
mod tests {
    #[test]
    fn banner_includes_version() {
        assert!(super::banner().ends_with(env!("CARGO_PKG_VERSION")));
    }
}
