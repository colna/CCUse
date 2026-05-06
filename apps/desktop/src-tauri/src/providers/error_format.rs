use std::error::Error as StdError;

pub(crate) fn format_error_chain(error: &(dyn StdError + 'static)) -> String {
    let mut message = error.to_string();
    let mut source = error.source();

    while let Some(err) = source {
        let rendered = err.to_string();
        if !rendered.is_empty() {
            message.push_str("; caused by: ");
            message.push_str(&rendered);
        }
        source = err.source();
    }

    message
}

pub(crate) fn format_reqwest_error(mut error: reqwest::Error) -> String {
    if let Some(url) = error.url_mut() {
        url.set_query(None);
        url.set_fragment(None);
    }

    format_error_chain(&error)
}

#[cfg(test)]
mod tests {
    use std::fmt;

    use super::*;

    #[derive(Debug)]
    struct LeafError;

    impl fmt::Display for LeafError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("socket refused")
        }
    }

    impl StdError for LeafError {}

    #[derive(Debug)]
    struct OuterError {
        source: LeafError,
    }

    impl fmt::Display for OuterError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("request failed")
        }
    }

    impl StdError for OuterError {
        fn source(&self) -> Option<&(dyn StdError + 'static)> {
            Some(&self.source)
        }
    }

    #[test]
    fn format_error_chain_includes_nested_source_messages() {
        let err = OuterError { source: LeafError };

        let rendered = format_error_chain(&err);

        assert_eq!(rendered, "request failed; caused by: socket refused");
    }

    #[tokio::test]
    async fn format_reqwest_error_redacts_query_and_keeps_cause_chain() {
        let err = reqwest::Client::new()
            .get("http://127.0.0.1:1/v1/models?key=sk-secret")
            .send()
            .await
            .expect_err("local port 1 should not accept test traffic");

        let rendered = format_reqwest_error(err);

        assert!(rendered.contains("error sending request"));
        assert!(rendered.contains("http://127.0.0.1:1/v1/models"));
        assert!(rendered.contains("caused by:"));
        assert!(!rendered.contains("sk-secret"));
    }
}
