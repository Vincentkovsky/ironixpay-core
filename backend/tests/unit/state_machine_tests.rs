use ironix_pay::entity::checkout_sessions::SessionStatus;

#[test]
fn test_determine_status_exact_amount() {
    let expected = 1000;
    let received = 1000;
    let threshold = 10;

    let status = SessionStatus::determine_by_amount(expected, received, threshold);
    assert_eq!(status, SessionStatus::Paid);
}

#[test]
fn test_determine_status_overpayment() {
    let expected = 1000;
    let received = 1050; // +50
    let threshold = 10;

    let status = SessionStatus::determine_by_amount(expected, received, threshold);
    assert_eq!(status, SessionStatus::Overpaid);
}

#[test]
fn test_determine_status_underpayment_within_threshold() {
    let expected = 1000;
    let received = 995; // -5 (within 10)
    let threshold = 10;

    let status = SessionStatus::determine_by_amount(expected, received, threshold);
    assert_eq!(status, SessionStatus::Paid);
}

#[test]
fn test_determine_status_underpayment_exceeding_threshold() {
    let expected = 1000;
    let received = 900; // -100 (exceeds 10)
    let threshold = 10;

    let status = SessionStatus::determine_by_amount(expected, received, threshold);
    assert_eq!(status, SessionStatus::Underpaid);
}

#[test]
fn test_is_terminal_states() {
    // Terminal states
    assert!(SessionStatus::Paid.is_terminal());
    assert!(SessionStatus::Overpaid.is_terminal());
    assert!(SessionStatus::Expired.is_terminal());

    // Non-terminal states
    assert!(!SessionStatus::Pending.is_terminal());
    assert!(!SessionStatus::Underpaid.is_terminal());
}

#[test]
fn test_transition_logic_simulation() {
    // Simulate CheckoutService logic:
    // "Skip if session is already in terminal state"

    let initial_status = SessionStatus::Paid;

    // Simulate an attempt to update (e.g. late incoming transaction)
    let should_update = !initial_status.is_terminal();

    assert!(!should_update, "Should NOT allow update from Paid state");

    // Conversely, from Underpaid
    let initial_status = SessionStatus::Underpaid;
    let should_update = !initial_status.is_terminal();

    assert!(should_update, "Should allow update from Underpaid state");
}

#[test]
fn test_is_successful() {
    assert!(SessionStatus::Paid.is_successful());
    assert!(SessionStatus::Overpaid.is_successful());

    assert!(!SessionStatus::Pending.is_successful());
    assert!(!SessionStatus::Underpaid.is_successful());
    assert!(!SessionStatus::Expired.is_successful());
}
