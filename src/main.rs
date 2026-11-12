//! riehub: One private hub for every AI model, tool, skill and workflow.

fn greeting() -> &'static str {
    "riehub is alive"
}

fn main() {
    println!("{}", greeting()); // rules-lint: allow print - seed binary, replaced by task T-0002
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn greeting_names_the_project() {
        assert!(greeting().starts_with("riehub"));
    }
}
