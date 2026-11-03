use clap::Parser;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use suricatax_rules::cli::{self, Cli, Commands};
use suricatax_rules::paths::PathProvider;
use suricatax_rules::rulesets::EnabledSource;

struct TestPaths(PathBuf);

impl TestPaths {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "suricatax-custom-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn source(&self, name: &str) -> PathBuf {
        self.sources_dir()
            .join(format!("{}.yaml", name.replace('/', "-")))
    }

    fn save_index(&self) {
        fs::create_dir_all(self.cache_dir()).unwrap();
        fs::write(
            self.cache_dir().join("index.yaml"),
            "version: 1\nsources:\n  et/open:\n    vendor: ET\n    summary: Test index source\n    url: https://example.test/%(__version__)s.rules\n",
        )
        .unwrap();
    }

    fn cache_rules(&self, url: &str, rules: &str) {
        fs::create_dir_all(self.cache_dir()).unwrap();
        fs::write(
            self.cache_dir()
                .join(format!("{:x}.tar.gz", md5::compute(url))),
            rules,
        )
        .unwrap();
    }
}

impl PathProvider for TestPaths {
    fn sources_dir(&self) -> PathBuf {
        self.0.join("sources")
    }
    fn cache_dir(&self) -> PathBuf {
        self.0.join("cache")
    }
    fn rules_dir(&self) -> PathBuf {
        self.0.join("rules")
    }
}

impl Drop for TestPaths {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn custom_source_lifecycle_without_index() {
    let paths = TestPaths::new();
    let name = "Local/team_1.rules";
    let url = "https://example.test/%(__version__)s.rules?foo=bar&v=%(__version__)s";
    cli::add_custom_ruleset(&paths, name, url).unwrap();
    assert_eq!(cli::enabled_rulesets(&paths).unwrap(), vec![name]);
    assert!(!paths.cache_dir().exists());

    let content = fs::read_to_string(paths.source(name)).unwrap();
    let yaml: serde_yaml::Value = serde_yaml::from_str(&content).unwrap();
    assert_eq!(yaml.as_mapping().unwrap().len(), 2);
    assert_eq!(yaml["source"].as_str(), Some(name));
    assert_eq!(yaml["url"].as_str(), Some(url));

    let err = cli::add_custom_ruleset(&paths, name, "https://example.test/new.rules").unwrap_err();
    assert!(err.to_string().contains("already enabled"));
    assert_eq!(fs::read_to_string(paths.source(name)).unwrap(), content);

    cli::disable_ruleset(&paths, name).unwrap();
    assert!(cli::enabled_rulesets(&paths).unwrap().is_empty());
    let disabled = paths.source(name).with_extension("yaml.disabled");
    assert!(disabled.exists());

    let new_url = "http://example.test/replacement.rules";
    cli::add_custom_ruleset(&paths, name, new_url).unwrap();
    let source: EnabledSource =
        serde_yaml::from_str(&fs::read_to_string(paths.source(name)).unwrap()).unwrap();
    assert_eq!(source.url.as_deref(), Some(new_url));
    assert!(!disabled.exists());
    // In particular, no stale disabled destination on Windows.
    cli::disable_ruleset(&paths, name).unwrap();
    assert!(cli::enabled_rulesets(&paths).unwrap().is_empty());
    assert!(!paths.cache_dir().exists());
}

#[test]
fn validates_names_before_writing() {
    let paths = TestPaths::new();
    for name in [
        "", "-foo", ".foo", "/foo", "a b", "é", "a\\b", "a:b", "a\nb",
    ] {
        assert!(
            cli::add_custom_ruleset(&paths, name, "https://example.test/rules").is_err(),
            "accepted {name:?}"
        );
    }
    assert!(!paths.sources_dir().exists());
    for name in ["a", "A-Z_0.1/path", "_local", "1", "feed.yaml", "a/../b"] {
        cli::add_custom_ruleset(&paths, name, "https://example.test/rules").unwrap();
        assert!(cli::enabled_rulesets(&paths)
            .unwrap()
            .contains(&name.into()));
        cli::disable_ruleset(&paths, name).unwrap();
    }
}

#[test]
fn validates_urls_before_writing() {
    let paths = TestPaths::new();
    for url in [
        "",
        "not a url",
        "/rules",
        "https://",
        "http://[invalid]/rules",
        "ftp://example.test/rules",
        "file:///rules",
        "data:text/plain,rules",
    ] {
        assert!(
            cli::add_custom_ruleset(&paths, "local", url).is_err(),
            "accepted {url:?}"
        );
    }
    assert!(!paths.sources_dir().exists());
}

#[test]
fn rejects_index_names_and_filename_collisions() {
    let paths = TestPaths::new();
    paths.save_index();
    for name in ["et/open", "et-open"] {
        let err =
            cli::add_custom_ruleset(&paths, name, "https://example.test/custom.rules").unwrap_err();
        assert!(err.to_string().contains("enable-ruleset et/open"));
    }
    assert!(!paths.sources_dir().exists());
    cli::add_custom_ruleset(&paths, "local/feed", "https://example.test/custom.rules").unwrap();
    let original = fs::read(paths.source("local/feed")).unwrap();
    assert!(cli::add_custom_ruleset(&paths, "local-feed", "https://example.test/other").is_err());
    assert_eq!(fs::read(paths.source("local/feed")).unwrap(), original);
    cli::disable_ruleset(&paths, "local/feed").unwrap();
    assert!(cli::add_custom_ruleset(&paths, "local-feed", "https://example.test/other").is_err());
    assert_eq!(
        fs::read(paths.source("local/feed").with_extension("yaml.disabled")).unwrap(),
        original
    );
}

#[test]
fn cli_add_list_and_disable() {
    let paths = TestPaths::new();
    let args = Cli::try_parse_from([
        "suricatax-rules",
        "--user",
        "add-source",
        "local/feed",
        "https://example.test/rules",
    ])
    .unwrap();
    assert!(matches!(args.command, Commands::AddSource { .. }));
    cli::run_with_path_provider(&args.command, &paths).unwrap();
    assert_eq!(cli::enabled_rulesets(&paths).unwrap(), vec!["local/feed"]);
    let args = Cli::try_parse_from(["suricatax-rules", "list-rulesets"]).unwrap();
    cli::run_with_path_provider(&args.command, &paths).unwrap();
    let args = Cli::try_parse_from(["suricatax-rules", "disable-ruleset", "local/feed"]).unwrap();
    cli::run_with_path_provider(&args.command, &paths).unwrap();
    assert!(cli::enabled_rulesets(&paths).unwrap().is_empty());
    assert!(Cli::try_parse_from(["suricatax-rules", "add-source", "local"]).is_err());
}

#[test]
fn custom_and_index_sources_merge_and_disabled_sources_are_excluded() {
    let paths = TestPaths::new();
    paths.save_index();
    cli::enable_ruleset(&paths, "et/open").unwrap();
    cli::add_custom_ruleset(&paths, "local", "https://example.test/custom.rules").unwrap();
    cli::add_custom_ruleset(&paths, "disabled", "https://example.test/disabled.rules").unwrap();
    cli::disable_ruleset(&paths, "disabled").unwrap();
    paths.cache_rules("https://example.test/8.0.1.rules", "alert ip any any -> any any (msg:\"old\"; sid:1; rev:1;)\nalert ip any any -> any any (msg:\"index\"; sid:2; rev:1;)\n");
    paths.cache_rules(
        "https://example.test/custom.rules",
        "alert ip any any -> any any (msg:\"new\"; sid:1; rev:2;)\n",
    );
    paths.cache_rules(
        "https://example.test/disabled.rules",
        "alert ip any any -> any any (msg:\"disabled\"; sid:3; rev:1;)\n",
    );
    cli::update_rules_with_suricata_version(&paths, false, true, Some("8.0.1")).unwrap();
    let output = fs::read_to_string(paths.rules_dir().join("suricata.rules")).unwrap();
    assert!(output.contains("msg:\"new\""));
    assert!(output.contains("msg:\"index\""));
    assert!(!output.contains("msg:\"old\""));
    assert!(!output.contains("msg:\"disabled\""));
}

#[test]
fn custom_update_downloads_resolved_url_without_index() {
    let paths = TestPaths::new();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = listener.local_addr().unwrap();
    let url = format!("http://{address}/%(__version__)s.rules?v=%(__version__)s");
    cli::add_custom_ruleset(&paths, "local", &url).unwrap();

    let server = std::thread::spawn(move || {
        // Bound the wait so a regression cannot leave a test server hanging.
        let deadline = Instant::now() + Duration::from_secs(10);
        for _ in 0..2 {
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(Instant::now() < deadline, "rules were not downloaded");
                        std::thread::sleep(Duration::from_millis(10));
                    }
                    Err(err) => panic!("accept failed: {err}"),
                }
            };
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut reader = BufReader::new(&stream);
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            assert_eq!(line, "GET /8.0.2.rules?v=8.0.2 HTTP/1.1\r\n");
            loop {
                line.clear();
                assert!(reader.read_line(&mut line).unwrap() > 0);
                if line == "\r\n" {
                    break;
                }
            }
            let body = "alert ip any any -> any any (msg:\"downloaded\"; sid:10; rev:1;)\n";
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
        }
    });

    // First update has no index; forced update must also ignore an unreadable index.
    cli::update_rules_with_suricata_version(&paths, false, true, Some("8.0.2")).unwrap();
    assert!(!paths.cache_dir().join("index.yaml").exists());
    fs::write(paths.cache_dir().join("index.yaml"), "invalid: [").unwrap();
    cli::update_rules_with_suricata_version(&paths, true, true, Some("8.0.2")).unwrap();
    server.join().unwrap();
    let output = fs::read_to_string(paths.rules_dir().join("suricata.rules")).unwrap();
    assert!(output.contains("msg:\"downloaded\""));
    cli::disable_ruleset(&paths, "local").unwrap();
    assert!(cli::enabled_rulesets(&paths).unwrap().is_empty());
}
