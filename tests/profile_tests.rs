use hpc_perf::types::Profile;

#[test]
fn test_profile_parsing() {
    assert_eq!(Profile::parse("battery"), Some(Profile::Battery));
    assert_eq!(Profile::parse("BAT"), Some(Profile::Battery));
    assert_eq!(Profile::parse("daily"), Some(Profile::Daily));
    assert_eq!(Profile::parse("ac"), Some(Profile::Daily));
    assert_eq!(Profile::parse("balanced"), Some(Profile::Daily));
    assert_eq!(Profile::parse("ai"), Some(Profile::Ai));
    assert_eq!(Profile::parse("cuda"), Some(Profile::Ai));
    assert_eq!(Profile::parse("gaming"), Some(Profile::Gaming));
    assert_eq!(Profile::parse("game"), Some(Profile::Gaming));
    assert_eq!(Profile::parse("vm-gaming"), Some(Profile::VmGaming));
    assert_eq!(Profile::parse("vfio"), Some(Profile::VmGaming));
    assert_eq!(Profile::parse("autovirt"), Some(Profile::VmGaming));
    assert_eq!(Profile::parse("invalid"), None);
}

#[test]
fn test_profile_display_and_str() {
    assert_eq!(Profile::Battery.as_str(), "battery");
    assert_eq!(Profile::Daily.as_str(), "daily");
    assert_eq!(Profile::Ai.as_str(), "ai");
    assert_eq!(Profile::Gaming.as_str(), "gaming");
    assert_eq!(Profile::VmGaming.as_str(), "vm-gaming");

    assert!(Profile::Battery.display_name().contains("Battery"));
    assert!(Profile::Gaming.display_name().contains("Gaming"));
}
