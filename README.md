# Suricatax Rule Manager

A minimal Suricata Rule Manager written in Rust.

The main purpose of this rule manager is to provide a minimal viable
rule manager so I can work with Suricata on Windows

Any resemblance to Suricata Update is coincidental, but I am trying to
keep it familiar and somewhat compatible for now.

## Custom rulesets

Add and immediately enable a ruleset from an HTTP or HTTPS URL:

```sh
suricatax-rules add-source local/feed 'https://example.com/%(__version__)s.rules'
suricatax-rules list-rulesets
suricatax-rules update --suricata-version 8.0.2
suricatax-rules disable-ruleset local/feed
```

Use `--user` for user-specific directories (the default on Windows). Names
must be non-empty and contain only ASCII letters, digits, `-`, `_`, `.`, or
`/`, and cannot start with `-`, `.`, or `/`. URLs may contain
`%(__version__)s`, which is replaced at download time using the same version
selection as index sources. Rule files and supported archives are handled
in the same way as index downloads.

Library callers can use `suricatax_rules::cli::add_custom_ruleset`:

```rust,ignore
pub fn add_custom_ruleset(
    path_provider: &dyn PathProvider,
    name: &str,
    url: &str,
) -> anyhow::Result<()>
```

Custom sources appear in `cli::enabled_rulesets` and can be disabled with
`cli::disable_ruleset`. Adding a custom source does not also enable `et/open`.
Adding, listing, disabling, and custom-only updates need no source-index
download. Updates containing index sources still refresh the index normally.
The existing `et/open` fallback still applies when no sources are enabled.

Sources use the existing, suricata-update-compatible storage format:
`sources/local-feed.yaml` (replace `/` in the name with `-`), containing:

```yaml
source: local/feed
url: https://example.com/%(__version__)s.rules
```

Disabling renames the file to `.yaml.disabled`. Re-adding a disabled custom
name replaces its URL and removes the disabled file. Enabled names and
names present in the locally cached index are rejected; use `enable-ruleset`
for index sources. Names that map to the same filename are also rejected to
avoid overwriting another source. There are no custom parameters or headers.

## Notice

Do not become dependent on this repo. It could be consumed by one of
my other projects such as EveBox, EveCtl... Or maybe become the new
Suricata-Update?
