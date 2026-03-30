# Acceptance Validation Report: `bt` CLI Stage 1

- Status: **PASS**
- Passed: **21/21**

| Check | Result | Notes |
|---|---|---|
| AC-T037-1 command surface | PASS | ok |
| AC-T037-2 subcommand help | PASS | ok |
| AC-T038-1 workspace init | PASS | ok |
| AC-T038-3 invalid workspace | PASS | warning: unused manifest key: profile.release.rand     Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.13s      Running `target/debug/ |
| AC-T039-1 scaffold structure | PASS | ok |
| AC-T039-3 duplicate rejection | PASS | warning: unused manifest key: profile.release.rand     Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.1 |
| AC-T038-2 project discovery | PASS | ok |
| AC-T039-2 sample strategy run | PASS | ok |
| AC-T040-1 registration generated | PASS | /tmp/bt_acc_hdaqwecy/projects/alpha/generated/strategy_registry.rs |
| AC-T040-2 deterministic generation | PASS | b5edcde41b81c2da1a22a63eb76a400825e6306e0ba530accc4868ea53cd97a5 vs b5edcde41b81c2da1a22a63eb76a400825e6306e0ba530accc4868ea53cd97a5 |
| AC-T040-3 invalid strategy handling | PASS | warning: unused manifest key: profile.release.rand     Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.13s      Running `target/debug/ |
| AC-T041-1 end-to-end run | PASS | see sample strategy run |
| AC-T041-2 parameter forwarding | PASS | ok |
| AC-T041-3 config precedence | PASS | ok |
| AC-T041-4 invalid data path | PASS | warning: unused manifest key: profile.release.rand     Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.1 |
| AC-T042-1 sweep execution | PASS | json emitted |
| AC-T042-2 reuse generation path | PASS | generated registry reused |
| AC-T042-3 invalid sweep config | PASS | warning: unused manifest key: profile.release.rand     Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.1 |
| AC-T044-1 contract doc present | PASS | ok |
| AC-T044-2 bt examples validated | PASS | workspace init + project init + run validated |
| AC-T044-3 stage boundary clarity | PASS | ok |
