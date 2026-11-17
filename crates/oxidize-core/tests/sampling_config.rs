//! Milestone 2 §1: the interleaved-measurement policy in `[performance]`.
use oxidize_core::Config;

#[test]
fn old_configs_and_checkpoints_get_the_default_measurement_policy() {
    let old: Config = toml::from_str("[performance]\niterations = 7\nwarmup = 2").unwrap();
    assert_eq!(old.performance.rounds, 3);
    assert_eq!(old.performance.max_relative_mad, 0.25);
    old.validate().unwrap();
    // A checkpoint's frozen config written before these fields existed.
    let mut json = serde_json::to_value(&old).unwrap();
    let performance = json["performance"].as_object_mut().unwrap();
    performance.remove("rounds");
    performance.remove("max_relative_mad");
    let restored: Config = serde_json::from_value(json).unwrap();
    assert_eq!(restored.performance.rounds, 3);
}

#[test]
fn the_policy_can_be_set_and_out_of_range_values_are_refused() {
    let set: Config = toml::from_str("[performance]\nrounds = 5\nmax_relative_mad = 0.1").unwrap();
    set.validate().unwrap();
    assert_eq!(set.performance.rounds, 5);
    for bad in [
        "rounds = 0",
        "rounds = 101",
        "max_relative_mad = 0.0",
        "max_relative_mad = -1.0",
        "max_relative_mad = 11.0",
        "max_relative_mad = nan",
        "max_relative_mad = inf",
    ] {
        let config: Config = toml::from_str(&format!("[performance]\n{bad}")).unwrap();
        assert!(config.validate().is_err(), "{bad}");
    }
    assert!(toml::from_str::<Config>("[performance]\nround = 3").is_err());
}

#[test]
fn whole_application_workloads_are_named_commands() {
    let config: Config = toml::from_str(
        "[[performance.workloads]]\nname = \"cli-30000\"\ncommand = [\"python3\", \"main.py\", \"30000\"]\n",
    )
    .unwrap();
    config.validate().unwrap();
    assert_eq!(config.performance.workloads[0].command[1], "main.py");
    assert!(Config::default().performance.workloads.is_empty());
    for bad in [
        "name = \"\"\ncommand = [\"x\"]",
        "name = \"has space\"\ncommand = [\"x\"]",
        "name = \"a\"\ncommand = []",
        "name = \"a\"\ncommand = [\"\"]",
    ] {
        let config: Config =
            toml::from_str(&format!("[[performance.workloads]]\n{bad}\n")).unwrap();
        assert!(config.validate().is_err(), "{bad}");
    }
    let twice: Config = toml::from_str(
        "[[performance.workloads]]\nname = \"a\"\ncommand = [\"x\"]\n[[performance.workloads]]\nname = \"a\"\ncommand = [\"y\"]\n",
    )
    .unwrap();
    assert!(twice.validate().is_err());
    assert!(
        toml::from_str::<Config>("[[performance.workloads]]\nname = \"a\"\ncmd = [\"x\"]\n")
            .is_err()
    );
}
