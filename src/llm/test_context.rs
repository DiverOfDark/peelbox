pub struct TestContext;

impl TestContext {
    /// Get current test name from environment variable or thread name
    pub fn current_test_name() -> Option<String> {
        if let Ok(test_name) = std::env::var("AIPACK_TEST_NAME") {
            if !test_name.is_empty() {
                return Some(Self::sanitize_test_name(&test_name)?);
            }
        }

        let thread = std::thread::current();
        let thread_name = thread.name()?;

        if thread_name.is_empty() || thread_name == "main" {
            return None;
        }

        Self::sanitize_test_name(thread_name)
    }

    /// Sanitize test name for filesystem use
    /// Converts "tests::test_name" -> "tests_test_name"
    /// Replaces special characters with underscores
    fn sanitize_test_name(thread_name: &str) -> Option<String> {
        let name = if thread_name.contains("::") {
            thread_name.split("::").collect::<Vec<_>>().join("_")
        } else {
            thread_name.to_string()
        };

        let sanitized: String = name
            .chars()
            .map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' })
            .collect();

        let trimmed = sanitized.trim_matches('_');
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    }

    pub fn is_test_context() -> bool {
        Self::current_test_name().is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_test_name() {
        assert_eq!(
            TestContext::sanitize_test_name("test_name"),
            Some("test_name".to_string())
        );

        assert_eq!(
            TestContext::sanitize_test_name("tests::test_name"),
            Some("tests_test_name".to_string())
        );

        assert_eq!(
            TestContext::sanitize_test_name("module::submodule::test_name"),
            Some("module_submodule_test_name".to_string())
        );

        assert_eq!(
            TestContext::sanitize_test_name("test-with-dashes"),
            Some("test_with_dashes".to_string())
        );
    }

    #[test]
    fn test_current_test_name_in_test() {
        let test_name = TestContext::current_test_name();
        assert!(test_name.is_some());
        let name = test_name.unwrap();
        assert!(name.contains("test_current_test_name_in_test"));
    }

    #[test]
    fn test_is_test_context() {
        assert!(TestContext::is_test_context());
    }
}
