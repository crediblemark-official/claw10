# Claw10 Crates Publishing Status

**Last updated:** 2026-07-20

## Published

| # | Crate | Package Name | Version |
|---|-------|-------------|---------|
| 1 | claw10-domain | claw10-domain | v0.3.0 |
| 2 | claw10-store | claw10-store | v0.3.0 |
| 3 | claw10-toon | claw10-toon | v0.3.0 |
| 4 | claw10-budget | claw10-budget | v0.3.0 |
| 5 | claw10-auth | claw10-auth | v0.3.0 |
| 6 | claw10-telemetry | claw10-telemetry | v0.3.0 |
| 7 | claw10-icvs | claw10-icvs | v0.3.0 |
| 8 | claw10-policy | claw10-policy | v0.3.0 |
| 9 | claw10-event | claw10-event | v0.3.0 |
| 10 | claw10-model-router | claw10-model-router | **v0.3.1** |
| 11 | claw10-lineage | claw10-lineage | v0.3.0 |
| 12 | claw10-lifecycle | claw10-lifecycle | v0.3.0 |
| 13 | claw10-mission | claw10-mission | v0.3.0 |
| 14 | claw10-task | claw10-task | v0.3.0 |
| 15 | claw10-skill | claw10-skill | v0.3.0 |
| 16 | claw10-context | claw10-context | v0.3.0 |
| 17 | claw10-scheduler | claw10-scheduler | v0.3.0 |
| 18 | claw10-memory | claw10-memory | v0.3.0 |
| 19 | claw10-worker | claw10-worker | v0.3.0 |
| 20 | claw10-prompt | claw10-prompt | v0.3.0 |
| 21 | claw10-artifact | claw10-artifact | v0.3.0 |
| 22 | claw10-gateway | claw10-gateway | v0.3.0 |
| 23 | claw10-tool | claw10-tool | v0.3.0 |
| 24 | claw10-agent | claw10-agent | v0.3.0 |
| 25 | claw10-spawn | claw10-spawn | v0.3.0 |
| 26 | claw10-control-api | claw10-control-api | v0.3.0 |
| 27 | claw10-tui | claw10-tui | v0.3.0 |
| 28 | claw10-cli | **claw10** | v0.3.0 |

**Total published:** 28/28 ✅

## Notes

- CLI crate package name is `claw10` (not `claw10-cli`)
- `claw10-model-router` bumped to v0.3.1 for `init_providers()` async function
- crates.io rate limit: max ~12 new crates per hour. Hit 429 multiple times.
- Assets fix: `assets/claw10.txt` copied into `crates/claw10-tui/assets/` for crates.io packaging
- Publish command: `cargo publish -p <name> --allow-dirty`
- Rate limit cooldown: ~10 min between batches of new crates
