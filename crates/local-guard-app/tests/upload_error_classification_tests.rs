//! Integration tests for upload failure classification.

use local_guard_upload::{FailureClass, UploadError, classify_upload_error};

#[test]
fn upload_error_classification_tests_distinguish_transient_and_permanent() {
    assert_eq!(
        classify_upload_error(&UploadError::Server(503)),
        FailureClass::Retriable
    );
    assert_eq!(
        classify_upload_error(&UploadError::Client(400)),
        FailureClass::Permanent
    );
}
