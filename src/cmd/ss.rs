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

//! The Linux `ss` source.
//!
//! `ss` is queried separately for IPv4 and IPv6 so wildcard addresses
//! such as `*:3000` retain their address family. Its output is treated
//! defensively: malformed lines are ignored, and sockets remain in the
//! result even when permissions hide their process metadata.

use std::error::Error;
use std::fmt;
use std::process::{Command, Output};

use crate::cmd::lsof::ListeningPort;

#[derive(Eq, PartialEq)]
pub struct SsError {
    reason: String,
}

impl Error for SsError {}

impl fmt::Debug for SsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.reason)
    }
}

impl fmt::Display for SsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

#[derive(Clone, Copy)]
enum IpVersion {
    V4,
    V6,
}

impl IpVersion {
    fn argument(self) -> &'static str {
        match self {
            Self::V4 => "-4",
            Self::V6 => "-6",
        }
    }

    fn type_name(self) -> &'static str {
        match self {
            Self::V4 => "IPv4",
            Self::V6 => "IPv6",
        }
    }
}

pub struct Ss;

impl Ss {
    /// Use `ss` to list listening TCP ports.
    ///
    /// IPv4 and IPv6 are queried independently. If one query fails, the
    /// successful family is still returned. Process metadata is optional
    /// because `ss` may omit it when the caller lacks permission.
    ///
    /// # Errors
    ///
    /// Errors if both `ss` queries fail.
    pub fn listening_ports() -> Result<Vec<ListeningPort>, SsError> {
        let ipv4 = Self::ss(IpVersion::V4);
        let ipv6 = Self::ss(IpVersion::V6);

        match (ipv4, ipv6) {
            (Ok(ipv4), Ok(ipv6)) => {
                let mut listening_ports = Self::parse(&ipv4, IpVersion::V4);
                listening_ports.extend(Self::parse(&ipv6, IpVersion::V6));
                Ok(listening_ports)
            }
            (Ok(ipv4), Err(_)) => Ok(Self::parse(&ipv4, IpVersion::V4)),
            (Err(_), Ok(ipv6)) => Ok(Self::parse(&ipv6, IpVersion::V6)),
            (Err(ipv4), Err(ipv6)) => Err(SsError {
                reason: format!("Both ss queries failed. IPv4: {ipv4} IPv6: {ipv6}"),
            }),
        }
    }

    #[cfg(not(tarpaulin_include))]
    fn ss(ip_version: IpVersion) -> Result<String, SsError> {
        #![allow(unreachable_code)]
        #[cfg(test)]
        {
            let fixture = match ip_version {
                IpVersion::V4 => "ss-v4.txt",
                IpVersion::V6 => "ss-v6.txt",
            };
            let fixture = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures")
                .join(fixture);
            let output = std::fs::read_to_string(fixture).expect("cannot read test fixture");
            return Ok(output);
        }

        let output = Command::new("ss")
            .args([
                "-H", // Do not print a header.
                "-O", // Keep each socket on one line.
                "-l", // Only listening sockets.
                "-n", // Do not resolve service names.
                "-t", // Only TCP sockets.
                "-p", // Include process metadata when permitted.
                "-e", // Include the owning UID and socket inode.
                ip_version.argument(),
            ])
            .output();

        match output {
            Ok(output) => Self::handle_output_ok(&output),
            Err(_) => Self::handle_output_err(),
        }
    }

    fn handle_output_ok(output: &Output) -> Result<String, SsError> {
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).into_owned())
        } else {
            Err(SsError {
                reason: "The ss command has failed in an unexpected way.".to_string(),
            })
        }
    }

    fn handle_output_err() -> Result<String, SsError> {
        Err(SsError {
            reason: "Unable to locate the ss executable on the system.".to_string(),
        })
    }

    fn parse(output: &str, ip_version: IpVersion) -> Vec<ListeningPort> {
        output
            .lines()
            .flat_map(|line| Self::parse_line(line, ip_version))
            .collect()
    }

    fn parse_line(line: &str, ip_version: IpVersion) -> Vec<ListeningPort> {
        let columns: Vec<&str> = line.split_ascii_whitespace().collect();
        let Some(state) = columns
            .iter()
            .position(|column| column.eq_ignore_ascii_case("LISTEN"))
        else {
            return Vec::new();
        };

        // Expected shape around the state:
        // LISTEN <recv-q> <send-q> <local-address> <peer-address>
        let Some(local_address) = columns.get(state + 3) else {
            return Vec::new();
        };
        let local_address = Self::normalize_local_address(local_address);
        let processes = Self::extract_processes(line);

        if processes.is_empty() {
            return vec![Self::new_listening_port(ip_version, local_address, "", "")];
        }

        processes
            .into_iter()
            .map(|(command, pid)| {
                Self::new_listening_port(ip_version, local_address.clone(), &command, &pid)
            })
            .collect()
    }

    fn normalize_local_address(address: &str) -> String {
        if let Some(port) = address.strip_prefix("0.0.0.0:") {
            return format!("*:{port}");
        }
        if let Some(port) = address.strip_prefix("[::]:") {
            return format!("*:{port}");
        }

        address.to_string()
    }

    fn extract_processes(line: &str) -> Vec<(String, String)> {
        let mut processes = Vec::new();
        let mut offset = 0;

        while let Some(relative_pid) = line[offset..].find("pid=") {
            let pid_start = offset + relative_pid + "pid=".len();
            let pid_end = line[pid_start..]
                .find(|character: char| !character.is_ascii_digit())
                .map_or(line.len(), |end| pid_start + end);

            if pid_start == pid_end {
                offset = pid_end;
                continue;
            }

            let before_pid = &line[..pid_start - "pid=".len()];
            let command = before_pid
                .rfind("\",")
                .and_then(|command_end| {
                    before_pid[..command_end]
                        .rfind("(\"")
                        .map(|command_start| &before_pid[command_start + 2..command_end])
                })
                .unwrap_or_default();
            let pid = &line[pid_start..pid_end];

            if !processes
                .iter()
                .any(|(_, existing_pid)| existing_pid == pid)
            {
                processes.push((command.to_string(), pid.to_string()));
            }

            offset = pid_end;
        }

        processes
    }

    fn new_listening_port(
        ip_version: IpVersion,
        name: String,
        command: &str,
        pid: &str,
    ) -> ListeningPort {
        let mut port = ListeningPort::new();
        port.command = command.to_string();
        port.pid = pid.to_string();
        port.type_ = ip_version.type_name().to_string();
        port.node = "TCP".to_string();
        port.name = name;
        port
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::process::ExitStatusExt;
    use std::process::ExitStatus;

    #[test]
    fn sserror_debug() {
        let error = SsError {
            reason: "an error has occurred".to_string(),
        };

        assert_eq!(format!("{error:?}"), "an error has occurred");
    }

    #[test]
    fn sserror_display() {
        let error = SsError {
            reason: "an error has occurred".to_string(),
        };

        assert_eq!(error.to_string(), "an error has occurred");
    }

    #[test]
    fn ss_successful_read() {
        let output = Output {
            status: ExitStatus::from_raw(0),
            stdout: b"<stdout>".to_vec(),
            stderr: b"<stderr>".to_vec(),
        };

        let result = Ss::handle_output_ok(&output).unwrap();

        assert_eq!(result, "<stdout>");
    }

    #[test]
    fn ss_unsuccessful_read() {
        let output = Output {
            status: ExitStatus::from_raw(1),
            stdout: b"<stdout>".to_vec(),
            stderr: b"<stderr>".to_vec(),
        };

        let error = Ss::handle_output_ok(&output).unwrap_err();

        assert_eq!(
            error,
            SsError {
                reason: "The ss command has failed in an unexpected way.".to_string(),
            }
        );
    }

    #[test]
    fn ss_error_with_command() {
        let error = Ss::handle_output_err().unwrap_err();

        assert_eq!(
            error,
            SsError {
                reason: "Unable to locate the ss executable on the system.".to_string(),
            }
        );
    }

    #[test]
    fn listening_ports() {
        let listening_ports = Ss::listening_ports().unwrap();

        assert!(listening_ports.iter().any(|port| {
            port.command == "next-server (v1"
                && port.pid == "651003"
                && port.type_ == "IPv6"
                && port.node == "TCP"
                && port.name == "*:3000"
        }));
    }

    #[test]
    fn parse_issue_1_output() {
        let output = r#"tcp   LISTEN 0      511                *:3000             *:*    users:(("next-server (v1",pid=651003,fd=24))"#;

        let listening_ports = Ss::parse(output, IpVersion::V6);

        assert_eq!(listening_ports.len(), 1);
        assert_eq!(listening_ports[0].command, "next-server (v1");
        assert_eq!(listening_ports[0].pid, "651003");
        assert_eq!(listening_ports[0].type_, "IPv6");
        assert_eq!(listening_ports[0].node, "TCP");
        assert_eq!(listening_ports[0].name, "*:3000");
    }

    #[test]
    fn parse_socket_without_process_metadata() {
        let output = "LISTEN 0 4096 127.0.0.53%lo:53 0.0.0.0:* uid:101 ino:12345";

        let listening_ports = Ss::parse(output, IpVersion::V4);

        assert_eq!(listening_ports.len(), 1);
        assert!(listening_ports[0].command.is_empty());
        assert!(listening_ports[0].pid.is_empty());
        assert_eq!(listening_ports[0].name, "127.0.0.53%lo:53");
    }

    #[test]
    fn parse_multiple_processes() {
        let output = r#"LISTEN 0 128 127.0.0.1:8000 0.0.0.0:* users:(("python3",pid=1234,fd=3),("python3",pid=1235,fd=3))"#;

        let listening_ports = Ss::parse(output, IpVersion::V4);

        assert_eq!(listening_ports.len(), 2);
        assert_eq!(listening_ports[0].pid, "1234");
        assert_eq!(listening_ports[1].pid, "1235");
    }

    #[test]
    fn parse_ignores_duplicate_process_descriptors() {
        let output = r#"LISTEN 0 128 127.0.0.1:8000 0.0.0.0:* users:(("python3",pid=1234,fd=3),("python3",pid=1234,fd=4))"#;

        let listening_ports = Ss::parse(output, IpVersion::V4);

        assert_eq!(listening_ports.len(), 1);
        assert_eq!(listening_ports[0].pid, "1234");
    }

    #[test]
    fn parse_ignores_non_listening_and_malformed_lines() {
        let output = "\
ESTAB 0 0 127.0.0.1:8000 127.0.0.1:40000
LISTEN missing columns
";

        assert!(Ss::parse(output, IpVersion::V4).is_empty());
    }

    #[test]
    fn normalize_unspecified_addresses() {
        assert_eq!(Ss::normalize_local_address("0.0.0.0:3000"), "*:3000");
        assert_eq!(Ss::normalize_local_address("[::]:3000"), "*:3000");
        assert_eq!(
            Ss::normalize_local_address("127.0.0.1:3000"),
            "127.0.0.1:3000"
        );
        assert_eq!(Ss::normalize_local_address("[::1]:3000"), "[::1]:3000");
    }
}
