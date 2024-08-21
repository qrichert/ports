// ports — List listening ports.
// Copyright (C) 2024  Quentin Richert
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.

use ports::cmd::{ListeningPort, Lsof, Ps};
use ports::ui;
use std::env;
use std::error::Error;
use std::fmt;

#[derive(Debug, Eq, PartialEq, PartialOrd)]
enum Mode {
    Regular,
    Verbose,
    VeryVerbose,
}

#[derive(Debug, Eq, PartialEq)]
struct Config {
    help: bool,
    version: bool,
    mode: Mode,
}

impl Config {
    fn new(args: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut config = Self {
            help: false,
            version: false,
            mode: Mode::Regular,
        };

        for arg in args.skip(1) {
            match arg.as_str() {
                "-h" | "--help" => {
                    config.help = true;
                    break;
                }
                "-v" | "--version" => {
                    config.version = true;
                    break;
                }
                "-vv" | "--verbose" => {
                    if config.mode >= Mode::Verbose {
                        continue; // Only increase verbosity.
                    }
                    config.mode = Mode::Verbose;
                }
                "-vvv" | "--very-verbose" => {
                    if config.mode >= Mode::VeryVerbose {
                        continue; // Only increase verbosity.
                    }
                    config.mode = Mode::VeryVerbose;
                }
                arg => {
                    return Err(format!("Unknown argument: '{arg}'"));
                }
            }
        }

        Ok(config)
    }
}

#[cfg(not(tarpaulin_include))]
fn main() -> Result<(), Box<dyn Error>> {
    let config = Config::new(env::args()).unwrap_or_else(|e| {
        eprintln!("{e}");
        help();
        std::process::exit(2);
    });

    if config.help {
        help();
        return Ok(());
    }
    if config.version {
        version();
        return Ok(());
    }

    run(&config)
}

#[cfg(not(tarpaulin_include))]
fn help() {
    print!(
        "\
{description}

Usage: {bin} [OPTIONS]

Options:
  -h, --help            Show this message and exit.
  -v, --version         Show the version and exit.
  -vv, --verbose        Additional process info.
  -vvv, --very-verbose  Even more extra info.
",
        description = env!("CARGO_PKG_DESCRIPTION"),
        bin = env!("CARGO_BIN_NAME"),
    );
}

#[cfg(not(tarpaulin_include))]
fn version() {
    println!("{} {}", env!("CARGO_BIN_NAME"), env!("CARGO_PKG_VERSION"));
}

#[cfg(not(tarpaulin_include))]
fn run(config: &Config) -> Result<(), Box<dyn Error>> {
    let listening_ports = Lsof::listening_ports()?;

    if listening_ports.is_empty() {
        return Ok(());
    }

    match config.mode {
        Mode::Regular => regular(listening_ports),
        Mode::Verbose => verbose(listening_ports),
        Mode::VeryVerbose => very_verbose(listening_ports),
    }
}

// Yes, bad, I know. But I want the same signature for all modes.
#[allow(clippy::needless_pass_by_value, clippy::unnecessary_wraps)]
#[cfg(not(tarpaulin_include))]
fn regular(listening_ports: Vec<ListeningPort>) -> Result<(), Box<dyn Error>> {
    let listening_ports: Vec<Vec<&String>> = listening_ports
        .iter()
        .map(|port| {
            vec![
                &port.command,
                &port.pid,
                &port.user,
                &port.type_,
                &port.node,
                &port.name,
            ]
        })
        .collect();

    print!(
        "{}",
        ui::Table::new()
            .headers(&["COMMAND", "PID", "USER", "TYPE", "NODE", "HOST:PORT"])
            .alignments(&[
                fmt::Alignment::Left,
                fmt::Alignment::Right,
                fmt::Alignment::Left,
                fmt::Alignment::Left,
                fmt::Alignment::Left,
                fmt::Alignment::Right
            ])
            .data(&listening_ports)
    );

    Ok(())
}

#[cfg(not(tarpaulin_include))]
fn verbose(mut listening_ports: Vec<ListeningPort>) -> Result<(), Box<dyn Error>> {
    // Enable more info through `ps aux`.
    let pids: Vec<&String> = listening_ports.iter().map(|port| &port.pid).collect();
    let processes_info = Ps::processes_info(&pids)?;

    for port in &mut listening_ports {
        port.enrich_with_process_info(&processes_info);
    }

    let empty = String::new();
    let listening_ports: Vec<Vec<&String>> = listening_ports
        .iter()
        .map(|port| {
            vec![
                &port.command,
                &port.pid,
                &port.user,
                &port.type_,
                &port.node,
                &port.name,
                port.pinfo.as_ref().map_or_else(|| &empty, |p| &p.command),
            ]
        })
        .collect();

    print!(
        "{}",
        ui::Table::new()
            .headers(&[
                "COMMAND",
                "PID",
                "USER",
                "TYPE",
                "NODE",
                "HOST:PORT",
                "COMMAND"
            ])
            .alignments(&[
                fmt::Alignment::Left,
                fmt::Alignment::Right,
                fmt::Alignment::Left,
                fmt::Alignment::Left,
                fmt::Alignment::Left,
                fmt::Alignment::Right,
                fmt::Alignment::Left,
            ])
            .data(&listening_ports)
    );

    Ok(())
}

#[cfg(not(tarpaulin_include))]
fn very_verbose(mut listening_ports: Vec<ListeningPort>) -> Result<(), Box<dyn Error>> {
    // Enable more info through `ps aux`.
    let pids: Vec<&String> = listening_ports.iter().map(|port| &port.pid).collect();
    let processes_info = Ps::processes_info(&pids)?;

    for port in &mut listening_ports {
        port.enrich_with_process_info(&processes_info);
    }

    let empty = String::new();
    let listening_ports: Vec<Vec<&String>> = listening_ports
        .iter()
        .map(|port| {
            vec![
                &port.command,
                &port.pid,
                &port.user,
                &port.type_,
                &port.node,
                &port.name,
                port.pinfo.as_ref().map_or_else(|| &empty, |p| &p.pc_cpu),
                port.pinfo.as_ref().map_or_else(|| &empty, |p| &p.pc_mem),
                port.pinfo.as_ref().map_or_else(|| &empty, |p| &p.start),
                port.pinfo.as_ref().map_or_else(|| &empty, |p| &p.time),
                port.pinfo.as_ref().map_or_else(|| &empty, |p| &p.command),
            ]
        })
        .collect();

    print!(
        "{}",
        ui::Table::new()
            .headers(&[
                "COMMAND",
                "PID",
                "USER",
                "TYPE",
                "NODE",
                "HOST:PORT",
                "%CPU",
                "%MEM",
                "START",
                "TIME",
                "COMMAND"
            ])
            .alignments(&[
                fmt::Alignment::Left,
                fmt::Alignment::Right,
                fmt::Alignment::Left,
                fmt::Alignment::Left,
                fmt::Alignment::Left,
                fmt::Alignment::Right,
                fmt::Alignment::Right,
                fmt::Alignment::Right,
                fmt::Alignment::Right,
                fmt::Alignment::Right,
                fmt::Alignment::Left,
            ])
            .data(&listening_ports)
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_no_args() {
        let args = vec![String::new()].into_iter();
        let config = Config::new(args).unwrap();

        assert_eq!(
            config,
            Config {
                help: false,
                version: false,
                mode: Mode::Regular,
            }
        );
    }

    #[test]
    fn config_with_bin_path() {
        let args = vec![String::from("/usr/local/bin/ports")].into_iter();
        let config = Config::new(args).unwrap();

        assert_eq!(
            config,
            Config {
                help: false,
                version: false,
                mode: Mode::Regular,
            }
        );
    }

    #[test]
    fn config_help_full() {
        let args = vec![String::new(), String::from("--help")].into_iter();
        let config = Config::new(args).unwrap();

        assert!(config.help);
    }

    #[test]
    fn config_help_short() {
        let args = vec![String::new(), String::from("-h")].into_iter();
        let config = Config::new(args).unwrap();

        assert!(config.help);
    }

    #[test]
    fn config_version_full() {
        let args = vec![String::new(), String::from("--version")].into_iter();
        let config = Config::new(args).unwrap();

        assert!(config.version);
    }

    #[test]
    fn config_version_short() {
        let args = vec![String::new(), String::from("-v")].into_iter();
        let config = Config::new(args).unwrap();

        assert!(config.version);
    }

    #[test]
    fn config_regular() {
        let args = vec![String::new()].into_iter();
        let config = Config::new(args).unwrap();

        assert_eq!(config.mode, Mode::Regular);
    }

    #[test]
    fn config_verbose_full() {
        let args = vec![String::new(), String::from("--verbose")].into_iter();
        let config = Config::new(args).unwrap();

        assert_eq!(config.mode, Mode::Verbose);
    }

    #[test]
    fn config_verbose_short() {
        let args = vec![String::new(), String::from("-vv")].into_iter();
        let config = Config::new(args).unwrap();

        assert_eq!(config.mode, Mode::Verbose);
    }

    #[test]
    fn config_verbose_over_verbose_is_no_op() {
        let args = vec![
            String::new(),
            String::from("--verbose"),
            String::from("--verbose"),
        ]
        .into_iter();
        let config = Config::new(args).unwrap();

        assert_eq!(config.mode, Mode::Verbose);
    }

    #[test]
    fn config_very_verbose_full() {
        let args = vec![String::new(), String::from("--very-verbose")].into_iter();
        let config = Config::new(args).unwrap();

        assert_eq!(config.mode, Mode::VeryVerbose);
    }

    #[test]
    fn config_very_verbose_short() {
        let args = vec![String::new(), String::from("-vvv")].into_iter();
        let config = Config::new(args).unwrap();

        assert_eq!(config.mode, Mode::VeryVerbose);
    }

    #[test]
    fn config_very_verbose_gt_verbose() {
        let args = vec![
            String::new(),
            String::from("--verbose"),
            String::from("--very-verbose"),
            String::from("--verbose"),
        ]
        .into_iter();
        let config = Config::new(args).unwrap();

        assert_eq!(config.mode, Mode::VeryVerbose);
    }

    #[test]
    fn config_very_verbose_over_very_verbose_is_no_op() {
        let args = vec![
            String::new(),
            String::from("--very-verbose"),
            String::from("--very-verbose"),
        ]
        .into_iter();
        let config = Config::new(args).unwrap();

        assert_eq!(config.mode, Mode::VeryVerbose);
    }

    #[test]
    fn config_bad_argument() {
        let args = vec![String::new(), String::from("--abcdef")].into_iter();
        let error = Config::new(args).unwrap_err();

        assert!(error.contains("'--abcdef'"));
    }
}
