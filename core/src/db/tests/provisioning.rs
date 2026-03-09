use super::*;

#[test]
fn test_collector_handoff_receipt_roundtrip() {
    init().ok();
    let package_id = format!("pkg-{}", uuid::Uuid::new_v4());
    let status = "consumed";
    assert!(record_collector_handoff_receipt(
        &package_id,
        Some(42),
        status,
        Some(7),
        Some("unit-test")
    )
    .is_ok());

    let receipts = list_collector_handoff_receipts(50).unwrap_or_default();
    let found = receipts
        .into_iter()
        .find(|r| r.package_id == package_id)
        .expect("expected inserted collector handoff receipt");
    assert_eq!(found.collector_row_id, Some(42));
    assert_eq!(found.status, status);
    assert_eq!(found.recommendation_id, Some(7));
}
