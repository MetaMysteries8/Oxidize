use oxidize_core::Config;

#[test]
fn file_effects_can_be_explicitly_enabled() {
    let parsed = toml::from_str::<Config>(include_str!(
        "../../../fixtures/python-file-effects/oxidize.toml"
    ));
    assert!(
        parsed.is_ok(),
        "explicit file-effect config is unsupported: {parsed:?}"
    );
    parsed.unwrap().validate().unwrap();
}

#[test]
fn file_effects_default_off_and_roundtrip_without_enabling_old_checkpoints() {
    let old: Config = serde_json::from_value(serde_json::json!({})).unwrap();
    assert!(old.verification.file_effect_paths.is_empty());
    let config: Config = toml::from_str(include_str!(
        "../../../fixtures/python-file-effects/oxidize.toml"
    ))
    .unwrap();
    let restored: Config = toml::from_str(&toml::to_string(&config).unwrap()).unwrap();
    assert_eq!(
        restored.verification.file_effect_paths,
        ["artifact.bin", "untouched.bin"]
    );
}

#[test]
fn unsafe_duplicate_or_unbounded_file_effect_paths_are_rejected() {
    for path in [
        "",
        "../outside",
        "/etc/passwd",
        "a\\b",
        ".git/config",
        ".oxidize/state.json",
        ".ssh/id_rsa",
        "a/.env.local",
        "key.pem",
    ] {
        let mut config = Config::default();
        config.verification.file_effect_paths = vec![path.into()];
        assert!(config.validate().is_err(), "{path}");
    }
    let mut config = Config::default();
    config.verification.file_effect_paths = vec!["output.bin".into(); 2];
    assert!(config.validate().is_err());
    config.verification.file_effect_paths = (0..33).map(|i| format!("file{i}.bin")).collect();
    assert!(config.validate().is_err());
    config.verification.file_effect_paths = vec!["x".repeat(1025)];
    assert!(config.validate().is_err());
    config.verification.file_effect_paths = vec!["output/result.bin".into()];
    assert!(config.validate().is_ok());
}
