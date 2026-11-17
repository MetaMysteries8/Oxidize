use oxidize_core::Config;

#[test]
fn comparison_policy_can_be_explicitly_enabled() {
    let parsed = toml::from_str::<Config>(include_str!(
        "../../../fixtures/python-comparison/oxidize.toml"
    ));
    assert!(
        parsed.is_ok(),
        "explicit opt-in comparison configuration must be supported: {parsed:?}"
    );
    parsed.unwrap().validate().unwrap();
}

#[test]
fn old_configs_and_json_checkpoints_default_to_exact_comparison() {
    let old: Config = toml::from_str("[verification]\ncases_file = 'cases.json'").unwrap();
    assert_eq!(
        old.verification.comparison,
        oxidize_core::ComparisonPolicy::default()
    );
    let mut json = serde_json::to_value(&old).unwrap();
    json["verification"]
        .as_object_mut()
        .unwrap()
        .remove("comparison");
    let restored: Config = serde_json::from_value(json).unwrap();
    assert_eq!(
        restored.verification.comparison,
        old.verification.comparison
    );
    let roundtrip: Config = toml::from_str(&toml::to_string(&old).unwrap()).unwrap();
    assert_eq!(
        roundtrip.verification.comparison,
        old.verification.comparison
    );
}

#[test]
fn invalid_tolerances_and_misspelled_options_are_rejected() {
    for limits in [
        "absolute = -1.0",
        "relative = -1.0",
        "absolute = nan",
        "relative = inf",
    ] {
        let config: Config = toml::from_str(&format!(
            "[verification.comparison.return_float_tolerance]\n{limits}\n"
        ))
        .unwrap();
        assert!(config.validate().is_err(), "{limits}");
    }
    for field in [
        "return_flot_tolerance = 0.01",
        "normalize_output_line_ending = true",
    ] {
        assert!(toml::from_str::<Config>(&format!("[verification.comparison]\n{field}")).is_err());
    }
}
