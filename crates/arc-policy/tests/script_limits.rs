use arc_policy::{PolicyLimits, ScriptPolicyLimits};
use arc_traffic::Policy;

#[test]
fn typecheck_defaults_are_valid_and_old_policy_wire_stays_three_class() {
    let p = Policy::default();
    let four = ScriptPolicyLimits::from(p);
    assert!(four.is_valid());
    assert_eq!(four.typecheck.window, 1);
    assert_eq!(four.typecheck.reservation.cpu_millis, 1000);
    assert_eq!(four.typecheck.reservation.memory_bytes, 1024 * 1024 * 1024);
    let old = PolicyLimits::from(p);
    assert_eq!(four.legacy(), old);
    let wire = serde_json::to_value(old).unwrap();
    assert_eq!(wire.as_object().unwrap().len(), 4);
    assert!(wire.get("typecheck").is_none());
    let wire = serde_json::to_value(four).unwrap();
    assert_eq!(
        serde_json::from_value::<ScriptPolicyLimits>(wire.clone()).unwrap(),
        four
    );
    assert!(serde_json::from_value::<PolicyLimits>(wire).is_err());
    let mut tuned = four;
    tuned.typecheck.window = 2;
    assert!(tuned.is_valid());
    let mut bad = four;
    bad.typecheck.window = 0;
    assert!(!bad.is_valid());
    bad = four;
    bad.typecheck.reservation.cpu_millis = bad.budget.cpu_millis + 1;
    assert!(!bad.is_valid());
    bad = four;
    bad.budget.memory_bytes = arc_core::meter::MAX_SAFE_INTEGER + 1;
    assert!(!bad.is_valid());
}
