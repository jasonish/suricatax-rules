// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Copyright 2025 Jason Ish <jason@codemonkey.net>

use clap::Parser;
use colored::Colorize;
use suricatax_rules::cli::Cli;

fn main() {
    if let Err(e) = suricatax_rules::run(Cli::parse()) {
        eprintln!("{}: {:#}", "Error".red().bold(), e);
        std::process::exit(1);
    }
}
