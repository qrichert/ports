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

//! Aggregate listening-port observations from platform tools.
//!
//! Linux queries both `lsof` and `ss`. Either source may be incomplete
//! even when it exits successfully, so their results are merged rather
//! than using one only when the other fails. Other platforms currently
//! use `lsof`.

use std::error::Error;
use std::fmt;

use crate::cmd::lsof::{ListeningPort, Lsof};
#[cfg(target_os = "linux")]
use crate::cmd::ps::Ps;
#[cfg(target_os = "linux")]
use crate::cmd::ss::Ss;

#[derive(Eq, PartialEq)]
pub struct ListeningPortsError {
    reason: String,
}

impl Error for ListeningPortsError {}

impl fmt::Debug for ListeningPortsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.reason)
    }
}

impl fmt::Display for ListeningPortsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

pub struct ListeningPorts;

impl ListeningPorts {
    /// List listening ports from every available platform source.
    ///
    /// On Linux, `lsof` and `ss` are both queried because a successful
    /// result from either command may still be incomplete. Results are
    /// combined when both work; one successful source is sufficient.
    ///
    /// # Errors
    ///
    /// Errors if no platform source succeeds.
    pub fn all() -> Result<Vec<ListeningPort>, ListeningPortsError> {
        #[cfg(target_os = "linux")]
        {
            let lsof = Lsof::listening_ports().map_err(|error| error.to_string());
            let ss = Ss::listening_ports().map_err(|error| error.to_string());
            let mut listening_ports = Self::aggregate(lsof, ss)?;

            // `ss` does not expose a username. Reuse the existing `ps`
            // source as best-effort enrichment without making socket
            // enumeration depend on it.
            let pids: Vec<&String> = listening_ports
                .iter()
                .filter_map(|port| (!port.pid.is_empty()).then_some(&port.pid))
                .collect();
            if let Ok(processes_info) = Ps::processes_info(&pids) {
                for port in &mut listening_ports {
                    port.enrich_with_process_info(&processes_info);
                }
            }

            return Ok(listening_ports);
        }

        #[cfg(not(target_os = "linux"))]
        {
            Lsof::listening_ports().map_err(|error| ListeningPortsError {
                reason: error.to_string(),
            })
        }
    }

    #[cfg(any(target_os = "linux", test))]
    fn aggregate(
        lsof: Result<Vec<ListeningPort>, String>,
        ss: Result<Vec<ListeningPort>, String>,
    ) -> Result<Vec<ListeningPort>, ListeningPortsError> {
        let mut listening_ports = match (lsof, ss) {
            (Ok(mut lsof), Ok(ss)) => {
                Self::merge(&mut lsof, ss);
                lsof
            }
            (Ok(lsof), Err(_)) => lsof,
            (Err(_), Ok(ss)) => ss,
            (Err(lsof), Err(ss)) => {
                return Err(ListeningPortsError {
                    reason: format!("Unable to list listening ports. lsof: {lsof} ss: {ss}"),
                });
            }
        };

        Self::sort(&mut listening_ports);
        Ok(listening_ports)
    }

    #[cfg(any(target_os = "linux", test))]
    fn merge(listening_ports: &mut Vec<ListeningPort>, additional: Vec<ListeningPort>) {
        for candidate in additional {
            if candidate.pid.is_empty() {
                if !listening_ports
                    .iter()
                    .any(|port| Self::same_socket(port, &candidate))
                {
                    listening_ports.push(candidate);
                }
                continue;
            }

            if let Some(existing) = listening_ports
                .iter_mut()
                .find(|port| port.pid == candidate.pid && Self::same_socket(port, &candidate))
            {
                Self::fill_missing(existing, candidate);
                continue;
            }

            // A source without process visibility may have contributed an
            // ownerless row for this socket. Replace it with the observed
            // owner rather than displaying both.
            listening_ports
                .retain(|port| !port.pid.is_empty() || !Self::same_socket(port, &candidate));
            listening_ports.push(candidate);
        }
    }

    #[cfg(any(target_os = "linux", test))]
    fn same_socket(left: &ListeningPort, right: &ListeningPort) -> bool {
        left.type_ == right.type_ && left.node == right.node && left.name == right.name
    }

    #[cfg(any(target_os = "linux", test))]
    fn fill_missing(existing: &mut ListeningPort, candidate: ListeningPort) {
        if existing.command.is_empty() {
            existing.command = candidate.command;
        }
        if existing.user.is_empty() {
            existing.user = candidate.user;
        }
        if existing.pinfo.is_none() {
            existing.pinfo = candidate.pinfo;
        }
    }

    #[cfg(any(target_os = "linux", test))]
    fn sort(listening_ports: &mut [ListeningPort]) {
        listening_ports.sort_by(|left, right| {
            let left_pid = left.pid.parse::<u32>().unwrap_or(u32::MAX);
            let right_pid = right.pid.parse::<u32>().unwrap_or(u32::MAX);

            left_pid
                .cmp(&right_pid)
                .then(left.type_.cmp(&right.type_))
                .then(left.name.cmp(&right.name))
                .then(left.command.cmp(&right.command))
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn port(pid: &str, type_: &str, name: &str, command: &str) -> ListeningPort {
        let mut port = ListeningPort::new();
        port.pid = pid.to_string();
        port.type_ = type_.to_string();
        port.node = "TCP".to_string();
        port.name = name.to_string();
        port.command = command.to_string();
        port
    }

    #[test]
    fn error_debug() {
        let error = ListeningPortsError {
            reason: "an error has occurred".to_string(),
        };

        assert_eq!(format!("{error:?}"), "an error has occurred");
    }

    #[test]
    fn error_display() {
        let error = ListeningPortsError {
            reason: "an error has occurred".to_string(),
        };

        assert_eq!(error.to_string(), "an error has occurred");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn all_combines_lsof_and_ss() {
        let listening_ports = ListeningPorts::all().unwrap();

        assert!(
            listening_ports
                .iter()
                .any(|port| port.pid == "2673" && port.name == "*:333")
        );
        assert!(
            listening_ports
                .iter()
                .any(|port| port.pid == "651003" && port.name == "*:3000")
        );
    }

    #[test]
    fn aggregate_combines_successful_sources() {
        let lsof = vec![port("100", "IPv4", "*:8000", "python")];
        let ss = vec![port("200", "IPv6", "*:3000", "next-server (v1")];

        let listening_ports = ListeningPorts::aggregate(Ok(lsof), Ok(ss)).unwrap();

        assert_eq!(listening_ports.len(), 2);
        assert!(listening_ports.iter().any(|port| port.pid == "100"));
        assert!(listening_ports.iter().any(|port| port.pid == "200"));
    }

    #[test]
    fn aggregate_uses_lsof_when_ss_fails() {
        let lsof = vec![port("100", "IPv4", "*:8000", "python")];

        let listening_ports =
            ListeningPorts::aggregate(Ok(lsof), Err("ss failed".to_string())).unwrap();

        assert_eq!(listening_ports.len(), 1);
        assert_eq!(listening_ports[0].pid, "100");
    }

    #[test]
    fn aggregate_uses_ss_when_lsof_fails() {
        let ss = vec![port("200", "IPv6", "*:3000", "next-server (v1")];

        let listening_ports =
            ListeningPorts::aggregate(Err("lsof failed".to_string()), Ok(ss)).unwrap();

        assert_eq!(listening_ports.len(), 1);
        assert_eq!(listening_ports[0].pid, "200");
    }

    #[test]
    fn aggregate_errors_when_both_sources_fail() {
        let error =
            ListeningPorts::aggregate(Err("lsof failed".to_string()), Err("ss failed".to_string()))
                .unwrap_err();

        assert!(error.to_string().contains("lsof failed"));
        assert!(error.to_string().contains("ss failed"));
    }

    #[test]
    fn merge_deduplicates_same_listener() {
        let mut listening_ports = vec![port("100", "IPv4", "*:8000", "python")];
        let additional = vec![port("100", "IPv4", "*:8000", "python3")];

        ListeningPorts::merge(&mut listening_ports, additional);

        assert_eq!(listening_ports.len(), 1);
        assert_eq!(listening_ports[0].command, "python");
    }

    #[test]
    fn merge_preserves_multiple_owners() {
        let mut listening_ports = vec![port("100", "IPv4", "*:8000", "nginx")];
        let additional = vec![port("200", "IPv4", "*:8000", "nginx")];

        ListeningPorts::merge(&mut listening_ports, additional);

        assert_eq!(listening_ports.len(), 2);
    }

    #[test]
    fn merge_replaces_ownerless_socket() {
        let mut listening_ports = vec![port("", "IPv6", "*:3000", "")];
        let additional = vec![port("200", "IPv6", "*:3000", "next-server (v1")];

        ListeningPorts::merge(&mut listening_ports, additional);

        assert_eq!(listening_ports.len(), 1);
        assert_eq!(listening_ports[0].pid, "200");
    }

    #[test]
    fn merge_keeps_ownerless_socket_when_no_owner_is_visible() {
        let mut listening_ports = Vec::new();
        let additional = vec![port("", "IPv6", "*:3000", "")];

        ListeningPorts::merge(&mut listening_ports, additional);

        assert_eq!(listening_ports.len(), 1);
        assert!(listening_ports[0].pid.is_empty());
    }

    #[test]
    fn sort_places_unknown_pids_last() {
        let mut listening_ports = vec![
            port("", "IPv4", "*:8000", ""),
            port("20", "IPv4", "*:3000", "node"),
            port("10", "IPv4", "*:4000", "python"),
        ];

        ListeningPorts::sort(&mut listening_ports);

        assert_eq!(listening_ports[0].pid, "10");
        assert_eq!(listening_ports[1].pid, "20");
        assert!(listening_ports[2].pid.is_empty());
    }
}
