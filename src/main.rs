use std::collections::HashSet;
use std::env;
use std::error::Error;
use std::fmt;
use std::thread;
use std::time::Duration;

use lessify::OutputPaged;
use verynicetable::Table;

use ports::{ListeningPort, ListeningPorts, ListeningPortsError};

// 300ms is a nice compromise as it's an eternity for a computer, and a
// reasonably fast worst-case for humans (even with run time added).
const WAIT_INTERVAL: Duration = Duration::from_millis(300);

#[derive(Debug, Eq, PartialEq, PartialOrd)]
enum Mode {
    Regular,
    Verbose,
    VeryVerbose,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WaitFor {
    None,
    Some,
    All,
}

#[derive(Debug, Eq, PartialEq)]
struct Config {
    help: bool,
    version: bool,
    mode: Mode,
    wait_for: Option<WaitFor>,
    // Set of ports that will be _retained_ in the output if non-empty.
    // If the set is empty, there won't be any filtering.
    port_filters: HashSet<u16>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            help: false,
            version: false,
            mode: Mode::Regular,
            wait_for: None,
            port_filters: HashSet::new(),
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
                "-V" | "--version" => {
                    config.version = true;
                    break;
                }
                "-v" | "--verbose" => {
                    if config.mode >= Mode::Verbose {
                        continue; // Only increase verbosity.
                    }
                    config.mode = Mode::Verbose;
                }
                "-vv" | "--very-verbose" => {
                    if config.mode >= Mode::VeryVerbose {
                        continue; // Only increase verbosity.
                    }
                    config.mode = Mode::VeryVerbose;
                }
                "-N" | "--wait-for-none" => config.set_wait_for(WaitFor::None)?,
                "-S" | "--wait-for-some" => config.set_wait_for(WaitFor::Some)?,
                "-A" | "--wait-for-all" => config.set_wait_for(WaitFor::All)?,
                // Single port (0-65535).
                port if let Ok(port) = port.parse::<u16>() => {
                    config.port_filters.insert(port);
                }
                // Range of ports (contains `-`).
                range
                    if let Some((Some(start), Some(end))) =
                        range.split_once('-').map(|range| {
                            (range.0.parse::<u16>().ok(), range.1.parse::<u16>().ok())
                        }) =>
                {
                    let range_start = std::cmp::min(start, end);
                    let range_end = std::cmp::max(start, end);

                    config.port_filters.extend(range_start..=range_end);
                }
                arg => {
                    return Err(format!("Unknown argument: '{arg}'"));
                }
            }
        }

        Ok(config)
    }

    fn set_wait_for(&mut self, wait_for: WaitFor) -> Result<(), String> {
        if self.wait_for.is_some_and(|current| current != wait_for) {
            return Err(String::from(
                "Only one '--wait-for-*' argument type can be used",
            ));
        }
        self.wait_for = Some(wait_for);
        Ok(())
    }
}

#[cfg(not(tarpaulin_include))]
fn main() -> Result<(), Box<dyn Error>> {
    let config = Config::new(env::args()).unwrap_or_else(|e| {
        eprintln!("fatal: {e}.");
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
  -v, --verbose         Additional process info.
  -vv, --very-verbose   Even more extra info.

  -N, --wait-for-none   Wait until no matching ports are listening.
  -S, --wait-for-some   Wait until at least one matching port is listening.
  -A, --wait-for-all    Wait until all matching ports are listening.

  -h, --help            Show this message and exit.
  -V, --version         Show the version and exit.
",
        description = env!("CARGO_PKG_DESCRIPTION"),
        bin = env!("CARGO_BIN_NAME"),
    );
}

#[cfg(not(tarpaulin_include))]
fn version() {
    println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
}

#[cfg(not(tarpaulin_include))]
fn run(config: &Config) -> Result<(), Box<dyn Error>> {
    let mut listening_ports = if let Some(wait_for) = config.wait_for {
        wait_for_listening_ports(wait_for, &config.port_filters)?
    } else {
        query_listening_ports(&config.port_filters)?
    };

    if listening_ports.is_empty() {
        return Ok(());
    }

    match config.mode {
        Mode::Regular => {
            // `ss` cannot always report user identity. Enrich only
            // affected rows, without making regular output depend on
            // the availability of `ps`.
            _ = ListeningPorts::enrich_missing_identity(&mut listening_ports);
            regular(&listening_ports);
        }
        Mode::Verbose => {
            ListeningPorts::enrich_process_info(&mut listening_ports)?;
            verbose(&listening_ports);
        }
        Mode::VeryVerbose => {
            ListeningPorts::enrich_process_info(&mut listening_ports)?;
            very_verbose(&listening_ports);
        }
    }

    Ok(())
}

#[cfg(not(tarpaulin_include))]
fn wait_for_listening_ports(
    wait_for: WaitFor,
    port_filters: &HashSet<u16>,
) -> Result<Vec<ListeningPort>, ListeningPortsError> {
    loop {
        let listening_ports = query_listening_ports(port_filters)?;

        if is_wait_condition_met(wait_for, port_filters, &listening_ports) {
            return Ok(listening_ports);
        }

        thread::sleep(WAIT_INTERVAL);
    }
}

fn is_wait_condition_met(
    wait_for: WaitFor,
    port_filters: &HashSet<u16>,
    listening_ports: &[ListeningPort],
) -> bool {
    match wait_for {
        WaitFor::None => listening_ports.is_empty(),
        WaitFor::Some => !listening_ports.is_empty(),
        WaitFor::All => {
            let listening_port_numbers: HashSet<u16> = listening_ports
                .iter()
                .filter_map(listening_port_number)
                .collect();
            if port_filters.is_empty() {
                // 1-65535
                (1..=u16::MAX).all(|port| listening_port_numbers.contains(&port))
            } else {
                port_filters.is_subset(&listening_port_numbers)
            }
        }
    }
}

#[cfg(not(tarpaulin_include))]
fn query_listening_ports(
    port_filters: &HashSet<u16>,
) -> Result<Vec<ListeningPort>, ListeningPortsError> {
    let mut listening_ports = ListeningPorts::all()?;

    if !port_filters.is_empty() {
        filter_ports(&mut listening_ports, port_filters);
    }

    Ok(listening_ports)
}

/// Retain only ports in the `allowed` set.
fn filter_ports(listening_ports: &mut Vec<ListeningPort>, allowed: &HashSet<u16>) {
    listening_ports
        .retain(|port| listening_port_number(port).is_some_and(|port| allowed.contains(&port)));
}

fn listening_port_number(listening_port: &ListeningPort) -> Option<u16> {
    let mut listening_on = listening_port.name.as_str(); // '1337'

    // If the port is given in the form '*:1337', extract it.
    if let Some((_, port)) = listening_on.rsplit_once(':') {
        listening_on = port;
    }

    listening_on.parse::<u16>().ok()
}

#[cfg(not(tarpaulin_include))]
fn regular(listening_ports: &[ListeningPort]) {
    let mut listening_ports: Vec<Vec<&String>> = listening_ports
        .iter()
        .map(|port| vec![&port.command, &port.pid, &port.user, &port.name])
        .collect();

    // Without type (IPv4/IPv6), some lines appear duplicated.
    listening_ports.dedup();

    Table::new()
        .headers(&["COMMAND", "PID", "USER", "HOST:PORT"])
        .alignments(&[
            fmt::Alignment::Left,
            fmt::Alignment::Right,
            fmt::Alignment::Left,
            fmt::Alignment::Right,
        ])
        .data(&listening_ports)
        .output_paged();
}

#[cfg(not(tarpaulin_include))]
fn verbose(listening_ports: &[ListeningPort]) {
    let empty = String::new();
    let listening_ports: Vec<Vec<&String>> = listening_ports
        .iter()
        .map(|port| {
            vec![
                &port.command,
                &port.pid,
                &port.user,
                &port.type_,
                &port.name,
                port.pinfo.as_ref().map_or_else(|| &empty, |p| &p.command),
            ]
        })
        .collect();

    Table::new()
        .headers(&["COMMAND", "PID", "USER", "TYPE", "HOST:PORT", "COMMAND"])
        .alignments(&[
            fmt::Alignment::Left,
            fmt::Alignment::Right,
            fmt::Alignment::Left,
            fmt::Alignment::Left,
            fmt::Alignment::Right,
            fmt::Alignment::Left,
        ])
        .data(&listening_ports)
        .output_paged();
}

#[cfg(not(tarpaulin_include))]
fn very_verbose(listening_ports: &[ListeningPort]) {
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
            "COMMAND",
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
        .output_paged();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn port(port: u16) -> ListeningPort {
        let mut listening_port = ListeningPort::new();
        listening_port.name = format!("*:{port}");
        listening_port
    }

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
                wait_for: None,
                port_filters: HashSet::new(),
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
                wait_for: None,
                port_filters: HashSet::new(),
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
        let args = vec![String::new(), String::from("-V")].into_iter();
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
    fn config_wait_for_none() {
        let args = vec![String::new(), String::from("--wait-for-none")].into_iter();
        let config = Config::new(args).unwrap();

        assert_eq!(config.wait_for, Some(WaitFor::None));
    }

    #[test]
    fn config_wait_for_none_short() {
        let args = vec![String::new(), String::from("-N")].into_iter();
        let config = Config::new(args).unwrap();

        assert_eq!(config.wait_for, Some(WaitFor::None));
    }

    #[test]
    fn config_wait_for_some() {
        let args = vec![String::new(), String::from("--wait-for-some")].into_iter();
        let config = Config::new(args).unwrap();

        assert_eq!(config.wait_for, Some(WaitFor::Some));
    }

    #[test]
    fn config_wait_for_some_short() {
        let args = vec![String::new(), String::from("-S")].into_iter();
        let config = Config::new(args).unwrap();

        assert_eq!(config.wait_for, Some(WaitFor::Some));
    }

    #[test]
    fn config_wait_for_all() {
        let args = vec![String::new(), String::from("--wait-for-all")].into_iter();
        let config = Config::new(args).unwrap();

        assert_eq!(config.wait_for, Some(WaitFor::All));
    }

    #[test]
    fn config_wait_for_all_short() {
        let args = vec![String::new(), String::from("-A")].into_iter();
        let config = Config::new(args).unwrap();

        assert_eq!(config.wait_for, Some(WaitFor::All));
    }

    #[test]
    fn config_wait_for_none_duplicate_is_no_op() {
        let args = vec![
            String::new(),
            String::from("--wait-for-none"),
            String::from("--wait-for-none"),
        ]
        .into_iter();
        let config = Config::new(args).unwrap();

        assert_eq!(config.wait_for, Some(WaitFor::None));
    }

    #[test]
    fn config_wait_for_some_duplicate_is_no_op() {
        let args = vec![
            String::new(),
            String::from("--wait-for-some"),
            String::from("--wait-for-some"),
        ]
        .into_iter();
        let config = Config::new(args).unwrap();

        assert_eq!(config.wait_for, Some(WaitFor::Some));
    }

    #[test]
    fn config_wait_for_all_duplicate_is_no_op() {
        let args = vec![
            String::new(),
            String::from("--wait-for-all"),
            String::from("--wait-for-all"),
        ]
        .into_iter();
        let config = Config::new(args).unwrap();

        assert_eq!(config.wait_for, Some(WaitFor::All));
    }

    #[test]
    fn config_wait_for_duplicate_short_and_long_is_no_op() {
        let args = vec![
            String::new(),
            String::from("-A"),
            String::from("--wait-for-all"),
        ]
        .into_iter();
        let config = Config::new(args).unwrap();

        assert_eq!(config.wait_for, Some(WaitFor::All));
    }

    #[test]
    fn config_wait_for_short_options_conflict() {
        let args = vec![String::new(), String::from("-N"), String::from("-S")].into_iter();
        let error = Config::new(args).unwrap_err();

        assert_eq!(error, "Only one '--wait-for-*' argument type can be used");
    }

    #[test]
    fn config_wait_for_none_conflicts_with_some() {
        let args = vec![
            String::new(),
            String::from("--wait-for-none"),
            String::from("--wait-for-some"),
        ]
        .into_iter();
        let error = Config::new(args).unwrap_err();

        assert_eq!(error, "Only one '--wait-for-*' argument type can be used");
    }

    #[test]
    fn config_wait_for_none_conflicts_with_all() {
        let args = vec![
            String::new(),
            String::from("--wait-for-none"),
            String::from("--wait-for-all"),
        ]
        .into_iter();
        let error = Config::new(args).unwrap_err();

        assert_eq!(error, "Only one '--wait-for-*' argument type can be used");
    }

    #[test]
    fn config_wait_for_some_conflicts_with_none() {
        let args = vec![
            String::new(),
            String::from("--wait-for-some"),
            String::from("--wait-for-none"),
        ]
        .into_iter();
        let error = Config::new(args).unwrap_err();

        assert_eq!(error, "Only one '--wait-for-*' argument type can be used");
    }

    #[test]
    fn config_wait_for_some_conflicts_with_all() {
        let args = vec![
            String::new(),
            String::from("--wait-for-some"),
            String::from("--wait-for-all"),
        ]
        .into_iter();
        let error = Config::new(args).unwrap_err();

        assert_eq!(error, "Only one '--wait-for-*' argument type can be used");
    }

    #[test]
    fn config_wait_for_all_conflicts_with_none() {
        let args = vec![
            String::new(),
            String::from("--wait-for-all"),
            String::from("--wait-for-none"),
        ]
        .into_iter();
        let error = Config::new(args).unwrap_err();

        assert_eq!(error, "Only one '--wait-for-*' argument type can be used");
    }

    #[test]
    fn config_wait_for_all_conflicts_with_some() {
        let args = vec![
            String::new(),
            String::from("--wait-for-all"),
            String::from("--wait-for-some"),
        ]
        .into_iter();
        let error = Config::new(args).unwrap_err();

        assert_eq!(error, "Only one '--wait-for-*' argument type can be used");
    }

    #[test]
    fn config_verbose_full() {
        let args = vec![String::new(), String::from("--verbose")].into_iter();
        let config = Config::new(args).unwrap();

        assert_eq!(config.mode, Mode::Verbose);
    }

    #[test]
    fn config_verbose_short() {
        let args = vec![String::new(), String::from("-v")].into_iter();
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
        let args = vec![String::new(), String::from("-vv")].into_iter();
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

        assert_eq!(config.port_filters, HashSet::from([1337, 42069]));
    }

    #[test]
    fn config_filters_normalizes_leading_zeros() {
        let args = vec![String::new(), String::from("01337")].into_iter();
        let config = Config::new(args).unwrap();

        assert_eq!(config.port_filters, HashSet::from([1337]));
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
            config.port_filters,
            HashSet::from([1000, 1001, 1002, 1003, 1004, 1005])
        );
    }

    #[test]
    fn config_range_filters_end_first() {
        let args = vec![String::new(), String::from("1005-1000")].into_iter();
        let config = Config::new(args).unwrap();

        assert_eq!(
            config.port_filters,
            HashSet::from([1000, 1001, 1002, 1003, 1004, 1005])
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
            config.port_filters,
            HashSet::from([
                1000, 1001, 1002, 1003, 1004, 1005, 40000, 40001, 40002, 40003
            ])
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
            config.port_filters,
            HashSet::from([8000, 1000, 1001, 1002, 1003, 1004, 1005])
        );
    }

    #[test]
    fn config_range_filters_range_equals() {
        let args = vec![String::new(), String::from("1000-1000")].into_iter();
        let config = Config::new(args).unwrap();

        assert_eq!(config.port_filters, HashSet::from([1000]));
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

        filter_ports(&mut listening_ports, &HashSet::from([1337, 42069]));

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

        filter_ports(&mut listening_ports, &HashSet::new());

        // This is correct. We happen to treat 'no-filters' as
        // 'keep-everything', but this is not `filter_ports()`' problem.
        assert!(listening_ports.is_empty());
    }

    #[test]
    fn wait_for_none_condition() {
        assert!(is_wait_condition_met(WaitFor::None, &HashSet::new(), &[],));
        assert!(!is_wait_condition_met(
            WaitFor::None,
            &HashSet::new(),
            &[ListeningPort::new()],
        ));
    }

    #[test]
    fn wait_for_some_condition() {
        assert!(!is_wait_condition_met(WaitFor::Some, &HashSet::new(), &[],));
        assert!(is_wait_condition_met(
            WaitFor::Some,
            &HashSet::new(),
            &[ListeningPort::new()],
        ));
    }

    #[test]
    fn wait_for_all_condition() {
        let port_filters = HashSet::from([3000, 8000, 8001]);
        assert!(!is_wait_condition_met(
            WaitFor::All,
            &port_filters,
            &[port(3000), port(3000), port(8000)],
        ));
        assert!(is_wait_condition_met(
            WaitFor::All,
            &port_filters,
            &[port(3000), port(8000), port(8001)],
        ));
    }

    #[test]
    fn wait_for_all_condition_without_filters() {
        let every_port: Vec<ListeningPort> = (1..=u16::MAX).map(port).collect();
        assert!(!is_wait_condition_met(
            WaitFor::All,
            &HashSet::new(),
            &[port(3000)],
        ));
        assert!(is_wait_condition_met(
            WaitFor::All,
            &HashSet::new(),
            &every_port,
        ));
    }
}
