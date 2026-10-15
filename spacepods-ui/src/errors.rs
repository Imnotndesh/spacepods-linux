//! Maps daemon and library errors to the text a person sees.
//!
//! The copy is chosen from the stable [`libspacepods::ErrorCode`], never from the
//! message text. Full detail goes to the log, so it is still available when someone
//! reports a problem.

use libspacepods::{Error, ErrorCode};

/// Plain-language text for an error.
pub fn user_text(err: &Error) -> &'static str {
    match err.code() {
        ErrorCode::NotConnected => "Your earbuds aren't connected.",
        ErrorCode::DeviceNotFound => "Couldn't find your earbuds. Put them in pairing mode and try again.",
        ErrorCode::BluetoothUnavailable => "Bluetooth is unavailable.",
        ErrorCode::BluetoothOff => "Bluetooth is turned off. Turn it on to connect your earbuds.",
        ErrorCode::Timeout => "Your earbuds didn't respond in time.",
        ErrorCode::Busy => "Still working on the last request.",
        ErrorCode::UnsupportedFeature => "Your earbuds don't support this.",
        ErrorCode::InvalidState => "This isn't available right now.",
        ErrorCode::InvalidArgument => "That value isn't valid.",
        ErrorCode::ProtocolMismatch => "SpacePods needs an update to work with its background service.",
        ErrorCode::ServiceUnavailable => "Can't reach the SpacePods background service.",
        // PayloadTooLarge, Internal, and any code added later.
        _ => "Something went wrong.",
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use libspacepods::ErrorCode;
    use std::time::Duration;

    /// Copy is for people, not developers: no crate names, CLI commands, or raw text.
    #[test]
    fn copy_is_plain_language() {
        let codes = [
            ErrorCode::NotConnected,
            ErrorCode::DeviceNotFound,
            ErrorCode::BluetoothUnavailable,
            ErrorCode::Timeout,
            ErrorCode::Busy,
            ErrorCode::UnsupportedFeature,
            ErrorCode::InvalidState,
            ErrorCode::InvalidArgument,
            ErrorCode::ProtocolMismatch,
            ErrorCode::ServiceUnavailable,
            ErrorCode::Internal,
        ];
        for code in codes {
            let err = Error::Service { code, message: "raw detail".into() };
            let text = user_text(&err);
            assert!(!text.is_empty());
            assert!(!text.contains("libspacepods"), "{text}");
            assert!(!text.contains("spacepods service"), "{text}");
            assert!(!text.contains("raw detail"), "{text}");
        }
    }

    #[test]
    fn timeouts_are_retryable_and_say_so() {
        let err = Error::Timeout(Duration::from_secs(3));
        assert!(err.retryable());
        assert_eq!(user_text(&err), "Your earbuds didn't respond in time.");
    }
}
