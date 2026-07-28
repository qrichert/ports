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

//! The `lsof` listening-port source.
//!
//! # Known platform quirks
//!
//! Linux `lsof` releases before 4.99.7 can silently omit a process when
//! its name contains unbalanced parentheses. This affects Next.js 16:
//! Linux truncates its task name to `next-server (v1`, which triggered
//! the buggy `/proc/<pid>/stat` parser. See:
//!
//! - <https://github.com/qrichert/ports/issues/1>
//! - <https://github.com/lsof-org/lsof/issues/359>
//! - <https://github.com/lsof-org/lsof/commit/b46fc8f6af1726f3f30947ceca1cd14e802e0874>
//!
//! The higher-level aggregator therefore also queries `ss` on Linux,
//! even when `lsof` exits successfully.

use std::error::Error;
use std::fmt;
use std::process::{Command, Output};
use std::str::Lines;

use crate::cmd::ps::ProcessInfo;

#[derive(Eq, PartialEq)]
pub struct LsofError {
    reason: &'static str,
}

impl Error for LsofError {}

impl fmt::Debug for LsofError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.reason)
    }
}

impl fmt::Display for LsofError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ListeningPort {
    pub command: String,
    pub pid: String,
    pub user: String,
    pub type_: String,
    pub node: String,
    pub name: String,
    pub pinfo: Option<ProcessInfo>,
    _cannot_instantiate: std::marker::PhantomData<()>,
}

impl PartialOrd for ListeningPort {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ListeningPort {
    /// Sort by PID first, and then type (IPv4/IPv6).
    ///
    /// This enables easy line de-duplication in output, on top of
    /// deterministic ordering.
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.pid
            .parse::<u32>()
            .unwrap_or(u32::MAX)
            .cmp(&other.pid.parse::<u32>().unwrap_or(u32::MAX))
            .then(self.type_.cmp(&other.type_))
    }
}

impl ListeningPort {
    #[must_use]
    pub fn new() -> Self {
        Self {
            command: String::new(),
            pid: String::new(),
            user: String::new(),
            type_: String::new(),
            node: String::new(),
            name: String::new(),
            pinfo: None,
            _cannot_instantiate: std::marker::PhantomData,
        }
    }

    pub fn enrich_with_process_info(&mut self, process_info: &[ProcessInfo]) {
        let pinfo = process_info.iter().find(|process| process.pid == self.pid);
        if let Some(pinfo) = pinfo {
            if self.command.is_empty() {
                self.command = pinfo
                    .command
                    .split_ascii_whitespace()
                    .next()
                    .and_then(|command| command.rsplit('/').next())
                    .unwrap_or_default()
                    .to_string();
            }
            if self.user.is_empty() {
                self.user.clone_from(&pinfo.user);
            }
            self.pinfo = Some(pinfo.clone());
        }
    }
}

impl Default for ListeningPort {
    fn default() -> Self {
        Self::new()
    }
}

pub struct Lsof;

impl Lsof {
    /// Use `lsof` to list listening ports.
    ///
    /// # Errors
    ///
    /// Errors if the `lsof` executable is not found, or if the command
    /// exits with a non-zero exit code.
    pub fn listening_ports() -> Result<Vec<ListeningPort>, LsofError> {
        let output = Self::lsof()?;
        let mut output = output.lines();

        let header_columns = Self::extract_header_columns(&mut output)?;
        let detail_lines = Self::extract_detail_lines_of_listening_ports(&mut output);

        let mut listening_ports =
            Self::map_detail_values_to_properties(&header_columns, &detail_lines);

        // Regular sort should be faster than unstable sort here because
        // the output of `lsof` is already sorted by PID (see docs).
        listening_ports.sort();

        Ok(listening_ports)
    }

    #[cfg(not(tarpaulin_include))]
    fn lsof() -> Result<String, LsofError> {
        #![allow(unreachable_code)]
        #[cfg(test)]
        {
            let fixture = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/lsof.txt");
            let output = std::fs::read_to_string(fixture).expect("cannot read test fixture");
            return Ok(output);
        }

        // Note: The `-F` options doesn't have everything we need, or at
        // least not in a ready-to-print way.
        let output = Command::new("lsof")
            .arg("-i") // -i List IP sockets.
            .arg("-n") // -n Do not resolve hostnames (no DNS).
            .arg("-P") // -P Do not resolve port names (list port number instead of its name).
            .output();

        match output {
            Ok(output) => Self::handle_output_ok(&output),
            Err(_) => Self::handle_output_err(),
        }
    }

    fn handle_output_ok(output: &Output) -> Result<String, LsofError> {
        if output.status.success() {
            // Exit 0.
            Ok(String::from_utf8_lossy(&output.stdout).into_owned())
        } else {
            // Non-zero exit code.
            let exit_code = output.status.code();
            let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

            // Exit 1 can also mean "nothing found", in which case
            // stderr is empty. "Nothing found" is not an error for us.
            if (matches!(exit_code, Some(1)) || exit_code.is_none()) && stderr.trim().is_empty() {
                // Fake an exit 0 with just the mandatory headers.
                return Ok(Self::headers().join(" "));
            }

            Err(LsofError {
                reason: "The lsof command has failed in an unexpected way.",
            })
        }
    }

    fn handle_output_err() -> Result<String, LsofError> {
        Err(LsofError {
            reason: "Unable to locate the lsof executable on the system.",
        })
    }

    /// Extract first line as column titles.
    fn extract_header_columns(output: &mut Lines) -> Result<Vec<String>, LsofError> {
        let Some(header) = output.next() else {
            return Err(LsofError {
                reason: "The lsof output is missing the header.",
            });
        };
        let header = header.to_ascii_uppercase(); // To make sure.
        let header: Vec<&str> = header.split_ascii_whitespace().collect();

        if !Self::header_contains_all_properties(&header) {
            return Err(LsofError {
                reason: "The lsof output is missing expected properties.",
            });
        }

        Ok(header.iter().map(ToString::to_string).collect())
    }

    fn header_contains_all_properties(vec: &[&str]) -> bool {
        for col in Self::headers() {
            if !vec.contains(col) {
                return false;
            }
        }
        true
    }

    fn headers() -> &'static [&'static str] {
        &["COMMAND", "PID", "USER", "TYPE", "NODE", "NAME"]
    }

    /// Extract the rest of the output as detail lines.
    ///
    /// Details lines are split on whitespace to make columns.
    ///
    /// Only lines that contain `(LISTEN)` are kept, and the `(LISTEN)`
    /// column is dropped from the output (we don't need it, and it
    /// doesn't have a column header so it would mess up the structure).
    fn extract_detail_lines_of_listening_ports<'a>(output: &'a mut Lines) -> Vec<Vec<&'a str>> {
        output
            .filter_map(|line| {
                // To be kept, a line _must_ contain `(LISTEN)`
                // (i.e., one of the columns `== "(LISTEN)"`).
                let mut line: Vec<&str> = line.split_ascii_whitespace().collect();
                for i in 0..line.len() {
                    if line[i].eq_ignore_ascii_case("(LISTEN)") {
                        // This line contains `(LISTEN)`. Now remove the
                        // column from the line, and return the line.
                        line.remove(i);
                        return Some(line);
                    }
                }
                // Not a listening port.
                None
            })
            .collect()
    }

    /// Associate column values to struct properties.
    fn map_detail_values_to_properties(
        header_columns: &[String],
        detail_lines: &[Vec<&str>],
    ) -> Vec<ListeningPort> {
        if detail_lines.is_empty() {
            return Vec::new();
        }

        let mut lsof = Vec::with_capacity(detail_lines.len());

        // Each line is a `Vec` of columns (split on whitespace).
        for detail_line in detail_lines {
            if detail_line.len() < header_columns.len() {
                continue;
            }

            // Better to have wasted intermediate `String::new()`s than
            // drag `Option`s around (`String::new()` doesn't allocate
            // and is cheap).
            let mut port = ListeningPort::new();

            for col in 0..header_columns.len() {
                let value = String::from(detail_line[col]);

                match header_columns[col].as_str() {
                    "COMMAND" => port.command = value,
                    "PID" => port.pid = value,
                    "USER" => port.user = value,
                    "TYPE" => port.type_ = value,
                    "NODE" => port.node = value,
                    "NAME" => port.name = value,
                    _ => {}
                }
            }

            lsof.push(port);
        }

        lsof
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::process::ExitStatusExt;
    use std::process::ExitStatus;

    #[test]
    fn lsoferror_debug() {
        let error = LsofError {
            reason: "an error has occurred",
        };

        assert_eq!(format!("{error:?}"), "an error has occurred");
    }

    #[test]
    fn lsoferror_display() {
        let error = LsofError {
            reason: "an error has occurred",
        };

        assert_eq!(error.to_string(), "an error has occurred");
    }

    #[test]
    fn lsof_successful_read() {
        let output = Output {
            status: ExitStatus::from_raw(0),
            stdout: b"<stdout>".to_vec(),
            stderr: b"<stderr>".to_vec(),
        };

        let res = Lsof::handle_output_ok(&output).unwrap();

        assert_eq!(res, "<stdout>");
    }

    #[test]
    fn lsof_successful_unsuccessful_read() {
        // Exit 1 with empty output is OK (just means nothing found).
        let output = Output {
            status: ExitStatus::from_raw(1),
            stdout: b"<stdout>".to_vec(),
            stderr: b"".to_vec(),
        };

        let res = Lsof::handle_output_ok(&output).unwrap();

        assert_eq!(res, Lsof::headers().join(" "));
    }

    #[test]
    fn lsof_unsuccessful_read() {
        let output = Output {
            status: ExitStatus::from_raw(1),
            stdout: b"<stdout>".to_vec(),
            stderr: b"<stderr>".to_vec(),
        };

        let res = Lsof::handle_output_ok(&output).unwrap_err();

        assert_eq!(
            res,
            LsofError {
                reason: "The lsof command has failed in an unexpected way.",
            }
        );
    }

    #[test]
    fn lsof_error_with_command() {
        let res = Lsof::handle_output_err().unwrap_err();

        assert_eq!(
            res,
            LsofError {
                reason: "Unable to locate the lsof executable on the system.",
            }
        );
    }

    #[test]
    fn listeningport_default() {
        assert_eq!(ListeningPort::new(), ListeningPort::default());
    }

    #[test]
    fn listeningport_new() {
        let port = ListeningPort::new();

        assert_eq!(
            port,
            ListeningPort {
                command: String::new(),
                pid: String::new(),
                user: String::new(),
                type_: String::new(),
                node: String::new(),
                name: String::new(),
                pinfo: None,
                _cannot_instantiate: std::marker::PhantomData,
            }
        );
    }

    // The `Lsof::listening_ports()` should be integration tests. But at
    // this scale, it's easier like this.

    #[test]
    fn listening_ports() {
        let listening_ports = Lsof::listening_ports().unwrap();

        let port: ListeningPort = listening_ports
            .into_iter()
            .find(|x| x.pid == "2673")
            .unwrap();

        assert_eq!(
            port,
            ListeningPort {
                command: String::from("docker-pr"),
                pid: String::from("2673"),
                user: String::from("root"),
                type_: String::from("IPv4"),
                node: String::from("TCP"),
                name: String::from("*:333"),
                pinfo: None,
                _cannot_instantiate: std::marker::PhantomData,
            }
        );
    }

    #[test]
    fn extract_header_columns_regular() {
        let headers = Lsof::headers().join(" ");
        let output = format!("{headers}\n");
        let mut output = output.lines();

        let columns = Lsof::extract_header_columns(&mut output).unwrap();

        assert_eq!(columns, Lsof::headers());
    }

    #[test]
    fn extract_header_columns_error_empty_output() {
        let output = String::new();
        let mut output = output.lines();

        let error = Lsof::extract_header_columns(&mut output).unwrap_err();

        assert_eq!(
            error,
            LsofError {
                reason: "The lsof output is missing the header."
            }
        );
    }

    #[test]
    fn extract_header_columns_no_newline_after_only_headers() {
        let headers = Lsof::headers().join(" ");
        let mut output = headers.lines();

        let columns = Lsof::extract_header_columns(&mut output).unwrap();

        assert_eq!(columns, Lsof::headers());
    }

    #[test]
    fn extract_header_columns_error_no_header() {
        let output = String::from("\n");
        let mut output = output.lines();

        let error = Lsof::extract_header_columns(&mut output).unwrap_err();

        assert_eq!(
            error,
            LsofError {
                // This is considered an empty header line, and so falls
                // into this error, instead of "no header"
                reason: "The lsof output is missing expected properties."
            }
        );
    }

    #[test]
    fn extract_header_columns_with_additional_headers() {
        let mut headers = Lsof::headers().to_vec();
        headers.push("FOO");
        headers.push("BAR");
        headers.push("BAZ");

        let output = format!("{}\n", headers.join(" "));
        let mut output = output.lines();

        let columns = Lsof::extract_header_columns(&mut output).unwrap();

        assert_eq!(columns, headers);
    }

    #[test]
    fn extract_header_columns_error_with_missing_headers() {
        let mut headers = Lsof::headers().to_vec();
        headers.pop();

        let output = format!("{}\n", headers.join(" "));
        let mut output = output.lines();

        let error = Lsof::extract_header_columns(&mut output).unwrap_err();

        assert_eq!(
            error,
            LsofError {
                reason: "The lsof output is missing expected properties.",
            }
        );
    }

    #[test]
    fn extract_header_columns_wrong_character_case() {
        let headers = Lsof::headers().join(" ").to_lowercase();
        let mut output = headers.lines();

        let columns = Lsof::extract_header_columns(&mut output).unwrap();

        assert_eq!(columns, Lsof::headers());
    }

    #[test]
    fn extract_detail_lines_of_listening_ports_regular() {
        let output = "\
This is not included
This is included (LISTEN)
This is included too (LISTEN)
This is again not included
";
        let mut output = output.lines();

        let detail_lines = Lsof::extract_detail_lines_of_listening_ports(&mut output);

        assert_eq!(
            detail_lines,
            vec![
                vec!["This", "is", "included"],
                vec!["This", "is", "included", "too"],
            ]
        );
    }

    #[test]
    fn extract_detail_lines_of_listening_ports_case_insensitive() {
        let output = "\
This is not included
This is included (listen)
This is included too (lIsTeN)
This is again not included
";
        let mut output = output.lines();

        let detail_lines = Lsof::extract_detail_lines_of_listening_ports(&mut output);

        assert_eq!(
            detail_lines,
            vec![
                vec!["This", "is", "included"],
                vec!["This", "is", "included", "too"],
            ]
        );
    }

    #[test]
    fn map_detail_values_to_properties() {
        let header_columns = [
            String::from("COMMAND"),
            String::from("PID"),
            String::from("USER"),
            String::from("TYPE"),
            String::from("NODE"),
            String::from("NAME"),
        ];

        let detail_lines = [vec![
            "<command>",
            "<pid>",
            "<user>",
            "<type>",
            "<node>",
            "<name>",
        ]];

        let lsof = Lsof::map_detail_values_to_properties(&header_columns, &detail_lines);

        assert_eq!(
            lsof,
            vec![ListeningPort {
                command: String::from("<command>"),
                pid: String::from("<pid>"),
                user: String::from("<user>"),
                type_: String::from("<type>"),
                node: String::from("<node>"),
                name: String::from("<name>"),
                pinfo: None,
                _cannot_instantiate: std::marker::PhantomData
            }],
        );
    }

    #[test]
    fn map_detail_values_to_properties_no_detail_lines() {
        let header_columns = [
            String::from("COMMAND"),
            String::from("PID"),
            String::from("USER"),
            String::from("TYPE"),
            String::from("NODE"),
            String::from("NAME"),
        ];

        let detail_lines = [];

        let lsof = Lsof::map_detail_values_to_properties(&header_columns, &detail_lines);

        assert_eq!(lsof, vec![],);
    }

    #[test]
    fn map_detail_values_to_properties_skips_malformed_lines() {
        let header_columns = Lsof::headers()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        let detail_lines = [vec!["command", "123"]];

        let lsof = Lsof::map_detail_values_to_properties(&header_columns, &detail_lines);

        assert!(lsof.is_empty());
    }

    #[test]
    fn map_detail_values_to_properties_extra_columns_are_ignored() {
        let header_columns = [
            String::from("PID"),
            String::from("NOT"),
            String::from("IN"),
            String::from("HEADERS"),
        ];

        let detail_lines = [vec!["<pid>", "<not>", "<in>", "<headers>"]];

        let lsof = Lsof::map_detail_values_to_properties(&header_columns, &detail_lines);

        assert_eq!(
            lsof,
            vec![ListeningPort {
                command: String::new(),
                pid: String::from("<pid>"),
                user: String::new(),
                type_: String::new(),
                node: String::new(),
                name: String::new(),
                pinfo: None,
                _cannot_instantiate: std::marker::PhantomData
            }],
        );
    }

    #[test]
    fn enrich_with_process_info_regular() {
        let mut port = ListeningPort {
            command: String::from("docker-pr"),
            pid: String::from("2673"),
            user: String::from("root"),
            type_: String::from("IPv4"),
            node: String::from("TCP"),
            name: String::from("*:333"),
            pinfo: None,
            _cannot_instantiate: std::marker::PhantomData,
        };

        let mut process = ProcessInfo::new();
        process.user = String::from("root");
        process.pid = String::from("2673");
        process.pc_cpu = String::from("0.0");
        process.pc_mem = String::from("0.0");
        process.start = String::from("09:27");
        process.time = String::from("0:02");
        process.command = String::from(
            "/usr/bin/docker-proxy -proto tcp -host-ip 0.0.0.0 -host-port 333 -container-ip 172.19.0.4 -container-port 22",
        );

        let mut other_process = ProcessInfo::new();
        other_process.user = String::from("colord");
        other_process.pid = String::from("874");
        other_process.pc_cpu = String::from("0.0");
        other_process.pc_mem = String::from("0.1");
        other_process.start = String::from("09:27");
        other_process.time = String::from("0:00");
        other_process.command = String::from("/usr/libexec/colord");

        port.enrich_with_process_info(&[process.clone(), other_process]);

        let pinfo = port.pinfo.unwrap();
        assert_eq!(pinfo.pid, port.pid);
        assert_eq!(pinfo, process);
    }

    #[test]
    fn enrich_with_process_info_missing_process() {
        let mut port = ListeningPort {
            command: String::from("docker-pr"),
            pid: String::from("2673"),
            user: String::from("root"),
            type_: String::from("IPv4"),
            node: String::from("TCP"),
            name: String::from("*:333"),
            pinfo: None,
            _cannot_instantiate: std::marker::PhantomData,
        };

        let mut other_process = ProcessInfo::new();
        other_process.user = String::from("colord");
        other_process.pid = String::from("874");
        other_process.pc_cpu = String::from("0.0");
        other_process.pc_mem = String::from("0.1");
        other_process.start = String::from("09:27");
        other_process.time = String::from("0:00");
        other_process.command = String::from("/usr/libexec/colord");

        port.enrich_with_process_info(&[other_process]);

        assert!(port.pinfo.is_none());
    }

    #[test]
    fn enrich_with_process_info_fills_missing_identity() {
        let mut port = ListeningPort::new();
        port.pid = String::from("2673");

        let mut process = ProcessInfo::new();
        process.user = String::from("root");
        process.pid = String::from("2673");
        process.command = String::from("/usr/bin/docker-proxy --help");

        port.enrich_with_process_info(&[process]);

        assert_eq!(port.command, "docker-proxy");
        assert_eq!(port.user, "root");
        assert!(port.pinfo.is_some());
    }

    #[test]
    fn enrich_with_process_info_missing_no_processes() {
        let mut port = ListeningPort {
            command: String::from("docker-pr"),
            pid: String::from("2673"),
            user: String::from("root"),
            type_: String::from("IPv4"),
            node: String::from("TCP"),
            name: String::from("*:333"),
            pinfo: None,
            _cannot_instantiate: std::marker::PhantomData,
        };

        port.enrich_with_process_info(&[]);

        assert!(port.pinfo.is_none());
    }
}
