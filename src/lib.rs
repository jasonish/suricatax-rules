// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: Copyright 2025 Jason Ish <jason@codemonkey.net>

pub mod cli;
pub mod paths;
pub mod rulesets;
pub mod sources;
pub mod update;
pub mod user_agent;

use anyhow::Result;

pub(crate) fn http_client_builder() -> reqwest::blocking::ClientBuilder {
    // reqwest's no-provider TLS feature expects the application to select one.
    // Ignore an error here because it means a provider was already installed.
    let _ = rustls::crypto::ring::default_provider().install_default();
    reqwest::blocking::Client::builder()
}

pub fn run(cli: cli::Cli) -> Result<()> {
    cli::run(cli)
}
