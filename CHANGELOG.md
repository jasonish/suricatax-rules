# Changelog

## [0.6.0] - 2026-11-03

### Added
- Custom HTTP(S) rulesets through `add-source <name> <url>` and the public
  `cli::add_custom_ruleset` helper, with immediate enabling and URL validation
- `list-rulesets` command to list enabled index and custom sources
- Support for `%(__version__)s` in custom URLs using the existing Suricata
  version selection and download pipeline
- Custom-only rule updates without downloading or reading the source index
- Replacement of disabled custom sources when re-added with a new URL

### Fixed
- Include valid source names ending in `.yaml` in enabled-source listings

### Changed
- Replaced `zip` with `rawzip` and compatible compression decoders to retain
  the Rust 1.85 minimum supported version shared with Suricata 9.0.0
- Replaced deprecated `serde_yaml` with the maintained `yaml_serde` fork,
  removing the unmaintained `unsafe-libyaml` dependency

### Security
- Removed the `zip` dependency affected by RUSTSEC-2026-0367,
  RUSTSEC-2026-0368, RUSTSEC-2026-0369, and RUSTSEC-2026-0429

## [0.5.0] - 2026-08-24

### Changed
- Renamed the project from Suricasta Rules to Suricatax Rules
- Relicensed the project from AGPL-3.0-only to MIT
- Raised the minimum supported Rust version from 1.82 to 1.85
- Updated dependencies, including reqwest 0.13

### Security
- Updated dependencies to resolve reported RustSec advisories

## [0.4.0] - 2026-08-21

### Added
- Support for datasets referenced by rules, including safe path rewriting and
  cleanup of unreferenced managed dataset files
- A `--suricata-version` update option, with automatic version detection from
  `suricata -V` when no override is provided
- Repeatable `--disable-regex` and `--disable-substring` update filters for
  removing matching enabled rules from the final ruleset
- Public library helpers for invoking commands and rule updates with a custom
  path provider

### Fixed
- Detect downloaded source formats by content so tarballs, zip archives,
  gzipped rules, and plain rule files work regardless of URL or cache filename

## [0.3.1] - 2025-07-07

### Changed
- Made `-v/--verbose` and `--user` options global, allowing them to be
  specified before or after sub-commands
- Add Suricata-Update style user-agent

## [0.3.0] - 2025-06-25

### Added
- Validation to prevent enabling obsolete rulesets with clear error messages
- Filtering of obsolete and deprecated rulesets from interactive selection menu

### Changed
- Interactive ruleset selection now only displays active, maintained rulesets
- Users attempting to enable obsolete rulesets receive informative error messages

## [0.2.0] - 2025-06-25

### Added
- Native Windows CI testing job for better Windows support validation
- Automatic `--user` mode for Windows platforms

### Changed
- Windows now always runs in `--user` mode by default for proper path handling
- Moved main binary from `src/bin/suricasta-rules.rs` to `src/main.rs` for simpler project structure
- Enhanced CI pipeline with native Windows builds and testing

### Fixed
- Windows path resolution issues by enforcing user-mode operation

## [0.1.0] - 2025-06-25

Initial release.
