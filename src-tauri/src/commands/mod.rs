pub mod notes;
pub mod vault;

use log::{info, warn};

/// Log the outcome of a mutating command — action name, an optional id, and
/// whether it succeeded — then return the result unchanged. It never receives
/// note content or passphrases, so the action log is safe to share.
pub(crate) fn log_outcome<T>(
    action: &str,
    id: Option<&str>,
    result: Result<T, String>,
) -> Result<T, String> {
    match (&result, id) {
        (Ok(_), Some(id)) => info!("{action} id={id} ok"),
        (Ok(_), None) => info!("{action} ok"),
        (Err(error), Some(id)) => warn!("{action} id={id} failed: {error}"),
        (Err(error), None) => warn!("{action} failed: {error}"),
    }
    result
}

/// Log only a failure for a read command (successful reads are not logged, to
/// keep the action log focused) and return the result unchanged.
pub(crate) fn log_read_error<T>(action: &str, result: Result<T, String>) -> Result<T, String> {
    if let Err(error) = &result {
        warn!("{action} failed: {error}");
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_outcome_returns_ok_unchanged() {
        let result: Result<i32, String> = log_outcome("test_action", Some("note-1"), Ok(42));
        assert_eq!(result, Ok(42));
    }

    #[test]
    fn log_outcome_returns_err_unchanged() {
        let result: Result<i32, String> =
            log_outcome("test_action", None, Err("boom".to_string()));
        assert_eq!(result, Err("boom".to_string()));
    }

    #[test]
    fn log_read_error_returns_value_unchanged() {
        let ok: Result<i32, String> = log_read_error("read", Ok(1));
        assert_eq!(ok, Ok(1));

        let err: Result<i32, String> = log_read_error("read", Err("x".to_string()));
        assert_eq!(err, Err("x".to_string()));
    }
}
