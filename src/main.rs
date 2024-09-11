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

use std::env;
use std::error::Error;
use std::fmt;

use verynicetable::Table;

use ports::lsof::{ListeningPort, Lsof};
use ports::ps::Ps;

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
    filters: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            help: false,
            version: false,
            mode: Mode::Regular,
            filters: Vec::new(),
        }
    }
}

impl Config {
    fn new(args: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut config = Self::default();

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
                arg if arg.parse::<u16>().is_ok() => {
                    // 0-65535
                    config.filters.push(String::from(arg));
                }
                // TODO[refactor]: Once 'if let guard' feature drops.
                //   arg if let Some((Some(start), Some(end))) =
                //       arg.split_once('-').and_then(|range| {
                //           Some((range.0.parse::<u16>().ok(), range.1.parse::<u16>().ok()))
                //       }) =>
                arg if arg.split_once('-').is_some_and(|range| {
                    range.0.parse::<u16>().is_ok() && range.1.parse::<u16>().is_ok()
                }) =>
                {
                    // TODO: Unnecessary once previous TODO gets resolved.
                    let range = arg
                        .split_once('-')
                        .map(|x| (x.0.parse::<u16>().unwrap(), x.1.parse::<u16>().unwrap()))
                        .unwrap();

                    let range_start = std::cmp::min(range.0, range.1);
                    let range_end = std::cmp::max(range.0, range.1);

                    // The bigger the range, the more we allocate...
                    // But it doesn't look like a bottleneck on a human
                    // time scale. If it ever gets to be a problem,
                    // we'll need to handle ranges differently.
                    let ports: Vec<String> = (range_start..=range_end)
                        .map(|port| port.to_string())
                        .collect();

                    config.filters.extend(ports);
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

Usage: {bin} [OPTIONS] [PORT[-RANGE] ...]

Filters:
  Filter on ports by passing port numbers or port ranges.
  For example `{bin} 8000 8003` or `{bin} 8000-8005`.

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
    let mut listening_ports = Lsof::listening_ports()?;

    if !config.filters.is_empty() {
        filter_ports(&mut listening_ports, &config.filters);
    }

    if listening_ports.is_empty() {
        return Ok(());
    }

    match config.mode {
        Mode::Regular => regular(listening_ports),
        Mode::Verbose => verbose(listening_ports),
        Mode::VeryVerbose => very_verbose(listening_ports),
    }
}

fn filter_ports(listening_ports: &mut Vec<ListeningPort>, allowed: &[String]) {
    listening_ports.retain(|x| {
        let mut listening_on = x.name.as_str(); // '*:1337'
        if let Some((_, port)) = listening_on.rsplit_once(':') {
            listening_on = port;
        };
        allowed.contains(&listening_on.to_string())
    });
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
        Table::new()
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
        Table::new()
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
        Table::new()
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
                filters: Vec::new(),
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
                filters: Vec::new(),
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
    fn config_filters() {
        let args = vec![String::new(), String::from("1337"), String::from("42069")].into_iter();
        let config = Config::new(args).unwrap();

        assert_eq!(
            config.filters,
            &[String::from("1337"), String::from("42069")]
        );
    }

    #[test]
    fn config_filters_invalid_too_low() {
        let args = vec![String::new(), String::from("-1")].into_iter();
        let error = Config::new(args).unwrap_err();

        assert!(error.contains("'-1'"));
    }

    #[test]
    fn config_filters_invalid_too_high() {
        let args = vec![String::new(), String::from("65536")].into_iter();
        let error = Config::new(args).unwrap_err();

        assert!(error.contains("'65536'"));
    }

    #[test]
    fn config_filters_invalid_not_a_number() {
        let args = vec![String::new(), String::from("123nan")].into_iter();
        let error = Config::new(args).unwrap_err();

        assert!(error.contains("'123nan'"));
    }

    #[test]
    fn config_range_filters_regular() {
        let args = vec![String::new(), String::from("1000-1005")].into_iter();
        let config = Config::new(args).unwrap();

        assert_eq!(
            config.filters,
            &[
                String::from("1000"),
                String::from("1001"),
                String::from("1002"),
                String::from("1003"),
                String::from("1004"),
                String::from("1005"),
            ]
        );
    }

    #[test]
    fn config_range_filters_end_first() {
        let args = vec![String::new(), String::from("1005-1000")].into_iter();
        let config = Config::new(args).unwrap();

        assert_eq!(
            config.filters,
            &[
                String::from("1000"),
                String::from("1001"),
                String::from("1002"),
                String::from("1003"),
                String::from("1004"),
                String::from("1005"),
            ]
        );
    }

    #[test]
    fn config_range_filters_multiple_ranges() {
        let args = vec![
            String::new(),
            String::from("1000-1005"),
            String::from("40000-40003"),
        ]
        .into_iter();
        let config = Config::new(args).unwrap();

        assert_eq!(
            config.filters,
            &[
                String::from("1000"),
                String::from("1001"),
                String::from("1002"),
                String::from("1003"),
                String::from("1004"),
                String::from("1005"),
                String::from("40000"),
                String::from("40001"),
                String::from("40002"),
                String::from("40003"),
            ]
        );
    }

    #[test]
    fn config_range_filters_with_simple_filter() {
        let args = vec![
            String::new(),
            String::from("8000"),
            String::from("1005-1000"),
        ]
        .into_iter();
        let config = Config::new(args).unwrap();

        assert_eq!(
            config.filters,
            &[
                String::from("8000"),
                String::from("1000"),
                String::from("1001"),
                String::from("1002"),
                String::from("1003"),
                String::from("1004"),
                String::from("1005"),
            ]
        );
    }

    #[test]
    fn config_range_filters_range_equals() {
        let args = vec![String::new(), String::from("1000-1000")].into_iter();
        let config = Config::new(args).unwrap();

        assert_eq!(config.filters, &[String::from("1000"),]);
    }

    #[test]
    fn config_range_filters_invalid_too_low() {
        let args = vec![String::new(), String::from("-1-10")].into_iter();
        let error = Config::new(args).unwrap_err();

        assert!(error.contains("'-1-10'"));
    }

    #[test]
    fn config_range_filters_invalid_too_high() {
        let args = vec![String::new(), String::from("65530-65536")].into_iter();
        let error = Config::new(args).unwrap_err();

        assert!(error.contains("'65530-65536'"));
    }

    #[test]
    fn config_bad_argument() {
        let args = vec![String::new(), String::from("--abcdef")].into_iter();
        let error = Config::new(args).unwrap_err();

        assert!(error.contains("'--abcdef'"));
    }

    #[test]
    fn filter_ports_regular() {
        let mut port_1 = ListeningPort::new();
        port_1.name = String::from("*:1337");
        let mut port_2 = ListeningPort::new();
        port_2.name = String::from("127.0.0.1:1337");
        let mut port_3 = ListeningPort::new();
        port_3.name = String::from("[::1]:1337");
        let mut port_4 = ListeningPort::new();
        port_4.name = String::from("[::]:42069");
        let mut port_5 = ListeningPort::new();
        port_5.name = String::from("42069");

        let mut port_6 = ListeningPort::new();
        port_6.name = String::new();
        let mut port_7 = ListeningPort::new();
        port_7.name = String::from("abc");
        let mut port_8 = ListeningPort::new();
        port_8.name = String::from("def:");

        let mut listening_ports = vec![
            port_1.clone(),
            port_2.clone(),
            port_3.clone(),
            port_4.clone(),
            port_5.clone(),
            port_6.clone(),
            port_7.clone(),
            port_8.clone(),
        ];

        filter_ports(
            &mut listening_ports,
            &[String::from("1337"), String::from("42069")],
        );

        assert!(listening_ports.contains(&port_1));
        assert!(listening_ports.contains(&port_2));
        assert!(listening_ports.contains(&port_3));
        assert!(listening_ports.contains(&port_4));
        assert!(listening_ports.contains(&port_5));

        assert!(!listening_ports.contains(&port_6));
        assert!(!listening_ports.contains(&port_7));
        assert!(!listening_ports.contains(&port_8));
    }

    #[test]
    fn filter_ports_empty() {
        let mut port_1 = ListeningPort::new();
        port_1.name = String::from("*:1337");
        let mut port_2 = ListeningPort::new();
        port_2.name = String::from("127.0.0.1:1337");
        let mut port_3 = ListeningPort::new();
        port_3.name = String::from("[::1]:1337");

        let mut listening_ports = vec![port_1, port_2, port_3];

        filter_ports(&mut listening_ports, &[]);

        // This is correct. We happen to treat 'no-filters' as
        // 'keep-everything', but this is not `filter_ports()`' problem.
        assert!(listening_ports.is_empty());
    }
}
