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
//! Linux supplements a successful `lsof` result with PID-bearing
//! observations from `ss`, because either source may omit listeners
//! even when it exits successfully. Other platforms currently use
//! `lsof`.

use std::error::Error;
use std::fmt;

use crate::cmd::lsof::Lsof;
use crate::cmd::ps::Ps;
#[cfg(target_os = "linux")]
use crate::cmd::ss::Ss;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ListeningPort {
    pub command: String,
    pub pid: String,
    pub user: String,
    pub type_: String,
    pub node: String,
    pub name: String,
    pub pinfo: Option<ProcessInfo>,
    pub(super) _cannot_instantiate: std::marker::PhantomData<()>,
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProcessInfo {
    pub user: String,
    pub pid: String,
    pub pc_cpu: String,
    pub pc_mem: String,
    pub start: String,
    pub time: String,
    pub command: String,
    pub(super) _cannot_instantiate: std::marker::PhantomData<()>,
}

impl ProcessInfo {
    #[must_use]
    pub fn new() -> Self {
        Self {
            user: String::new(),
            pid: String::new(),
            pc_cpu: String::new(),
            pc_mem: String::new(),
            start: String::new(),
            time: String::new(),
            command: String::new(),
            _cannot_instantiate: std::marker::PhantomData,
        }
    }
}

impl Default for ProcessInfo {
    fn default() -> Self {
        Self::new()
    }
}

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
    /// On Linux, a successful `lsof` result is supplemented with
    /// PID-bearing observations from `ss`, because either command may
    /// omit listeners even when it exits successfully.
    ///
    /// # Errors
    ///
    /// Errors if `lsof` fails.
    pub fn all() -> Result<Vec<ListeningPort>, ListeningPortsError> {
        #[cfg(target_os = "linux")]
        {
            let (lsof, ss) = std::thread::scope(|scope| {
                let ss = scope.spawn(|| Ss::listening_ports().map_err(|error| error.to_string()));
                let lsof = Lsof::listening_ports().map_err(|error| error.to_string());
                let ss = match ss.join() {
                    Ok(ss) => ss,
                    Err(panic) => std::panic::resume_unwind(panic),
                };
                (lsof, ss)
            });
            Self::aggregate(lsof, ss)
        }

        #[cfg(not(target_os = "linux"))]
        {
            Lsof::listening_ports().map_err(|error| ListeningPortsError {
                reason: error.to_string(),
            })
        }
    }

    /// Add process metadata to listening-port observations.
    ///
    /// # Errors
    ///
    /// Errors if process information cannot be queried.
    pub fn enrich_process_info(
        listening_ports: &mut [ListeningPort],
    ) -> Result<(), ListeningPortsError> {
        Self::enrich_process_info_where(listening_ports, |port| !port.pid.is_empty())
    }

    /// Add process metadata where command or user identity is missing.
    ///
    /// Rows without a PID cannot be enriched and do not trigger a
    /// process query.
    ///
    /// # Errors
    ///
    /// Errors if process information cannot be queried.
    pub fn enrich_missing_identity(
        listening_ports: &mut [ListeningPort],
    ) -> Result<(), ListeningPortsError> {
        Self::enrich_process_info_where(listening_ports, |port| {
            !port.pid.is_empty() && (port.command.is_empty() || port.user.is_empty())
        })
    }

    fn enrich_process_info_where(
        listening_ports: &mut [ListeningPort],
        should_enrich: impl Fn(&ListeningPort) -> bool,
    ) -> Result<(), ListeningPortsError> {
        let mut pids: Vec<&String> = listening_ports
            .iter()
            .filter_map(|port| should_enrich(port).then_some(&port.pid))
            .collect();
        pids.sort_unstable();
        pids.dedup();

        if pids.is_empty() {
            return Ok(());
        }

        let processes_info = Ps::processes_info(&pids).map_err(|error| ListeningPortsError {
            reason: error.to_string(),
        })?;

        for port in listening_ports {
            if should_enrich(port) {
                port.enrich_with_process_info(&processes_info);
            }
        }

        Ok(())
    }

    #[cfg(any(target_os = "linux", test))]
    fn aggregate(
        lsof: Result<Vec<ListeningPort>, String>,
        ss: Result<Vec<ListeningPort>, String>,
    ) -> Result<Vec<ListeningPort>, ListeningPortsError> {
        let mut listening_ports = lsof.map_err(|reason| ListeningPortsError { reason })?;
        if let Ok(ss) = ss {
            Self::merge(&mut listening_ports, ss);
        }

        Self::sort(&mut listening_ports);
        Ok(listening_ports)
    }

    #[cfg(any(target_os = "linux", test))]
    fn merge(listening_ports: &mut Vec<ListeningPort>, additional: Vec<ListeningPort>) {
        for candidate in additional {
            if candidate.pid.is_empty() {
                continue;
            }

            if let Some(existing) = listening_ports
                .iter_mut()
                .find(|port| port.pid == candidate.pid && Self::same_socket(port, &candidate))
            {
                Self::fill_missing(existing, candidate);
                continue;
            }

            // If the primary source contributed an ownerless row for
            // this socket, replace it with the observed owner rather
            // than displaying both.
            listening_ports
                .retain(|port| !port.pid.is_empty() || !Self::same_socket(port, &candidate));
            listening_ports.push(candidate);
        }
    }

    #[cfg(any(target_os = "linux", test))]
    fn same_socket(left: &ListeningPort, right: &ListeningPort) -> bool {
        left.type_ == right.type_
            && left.node == right.node
            && Self::same_socket_name(&left.name, &right.name)
    }

    #[cfg(any(target_os = "linux", test))]
    fn same_socket_name(left: &str, right: &str) -> bool {
        if left == right {
            return true;
        }

        let (
            Some((left_address, left_scope, left_port)),
            Some((right_address, right_scope, right_port)),
        ) = (
            Self::split_socket_name(left),
            Self::split_socket_name(right),
        )
        else {
            return false;
        };

        left_address == right_address
            && left_port == right_port
            && (left_scope == right_scope || left_scope.is_none() || right_scope.is_none())
    }

    #[cfg(any(target_os = "linux", test))]
    fn split_socket_name(name: &str) -> Option<(&str, Option<&str>, &str)> {
        let (address, port) = name.rsplit_once(':')?;
        let (address, scope) = address
            .rsplit_once('%')
            .map_or((address, None), |(address, scope)| (address, Some(scope)));
        let address = address.strip_prefix('[').unwrap_or(address);
        let address = address.strip_suffix(']').unwrap_or(address);
        let scope = scope.map(|scope| scope.strip_suffix(']').unwrap_or(scope));

        Some((address, scope, port))
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
        listening_ports.sort();
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

    fn process(pid: &str, user: &str, command: &str) -> ProcessInfo {
        let mut process = ProcessInfo::new();
        process.pid = pid.to_string();
        process.user = user.to_string();
        process.command = command.to_string();
        process
    }

    #[test]
    fn listening_port_default() {
        assert_eq!(ListeningPort::new(), ListeningPort::default());
    }

    #[test]
    fn process_info_default() {
        assert_eq!(ProcessInfo::new(), ProcessInfo::default());
    }

    #[test]
    fn enrich_with_process_info() {
        let mut port = port("2673", "IPv4", "*:333", "docker-pr");
        port.user = "root".to_string();
        let process = process("2673", "root", "/usr/bin/docker-proxy --help");

        port.enrich_with_process_info(std::slice::from_ref(&process));

        assert_eq!(port.pinfo, Some(process));
    }

    #[test]
    fn enrich_with_process_info_fills_missing_identity() {
        let mut port = port("2673", "IPv4", "*:333", "");
        let process = process("2673", "root", "/usr/bin/docker-proxy --help");

        port.enrich_with_process_info(&[process]);

        assert_eq!(port.command, "docker-proxy");
        assert_eq!(port.user, "root");
        assert!(port.pinfo.is_some());
    }

    #[test]
    fn enrich_with_process_info_ignores_missing_process() {
        let mut port = port("2673", "IPv4", "*:333", "docker-pr");
        let other_process = process("874", "colord", "/usr/libexec/colord");

        port.enrich_with_process_info(&[other_process]);

        assert!(port.pinfo.is_none());
    }

    #[test]
    fn enrich_process_info_queries_ps() {
        let mut listening_ports = vec![port("2673", "IPv4", "*:333", "docker-pr")];

        ListeningPorts::enrich_process_info(&mut listening_ports).unwrap();

        assert_eq!(listening_ports[0].user, "root");
        assert_eq!(
            listening_ports[0]
                .pinfo
                .as_ref()
                .map(|info| info.pid.as_str()),
            Some("2673")
        );
    }

    #[test]
    fn enrich_process_info_skips_ownerless_rows() {
        let mut listening_ports = vec![port("", "IPv4", "127.0.0.53:53", "")];

        ListeningPorts::enrich_process_info(&mut listening_ports).unwrap();

        assert!(listening_ports[0].pinfo.is_none());
    }

    #[test]
    fn enrich_missing_identity_queries_ps_for_incomplete_rows() {
        let mut listening_ports = vec![port("2673", "IPv4", "*:333", "docker-pr")];

        ListeningPorts::enrich_missing_identity(&mut listening_ports).unwrap();

        assert_eq!(listening_ports[0].user, "root");
        assert!(listening_ports[0].pinfo.is_some());
    }

    #[test]
    fn enrich_missing_identity_skips_complete_rows() {
        let mut complete = port("2673", "IPv4", "*:333", "docker-pr");
        complete.user = "root".to_string();
        let mut listening_ports = vec![complete];

        ListeningPorts::enrich_missing_identity(&mut listening_ports).unwrap();

        assert!(listening_ports[0].pinfo.is_none());
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
        assert_eq!(
            listening_ports
                .iter()
                .filter(|port| {
                    port.type_ == "IPv4"
                        && port.node == "TCP"
                        && port.name.starts_with("127.0.0.53")
                })
                .count(),
            1
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
    fn aggregate_errors_when_lsof_fails() {
        let ss = vec![port("200", "IPv6", "*:3000", "next-server (v1")];

        let error = ListeningPorts::aggregate(Err("lsof failed".to_string()), Ok(ss)).unwrap_err();

        assert_eq!(error.to_string(), "lsof failed");
    }

    #[test]
    fn aggregate_preserves_lsof_error_when_both_sources_fail() {
        let error =
            ListeningPorts::aggregate(Err("lsof failed".to_string()), Err("ss failed".to_string()))
                .unwrap_err();

        assert_eq!(error.to_string(), "lsof failed");
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
    fn merge_reconciles_interface_scoped_address() {
        let mut listening_ports = vec![port("580", "IPv4", "127.0.0.53:53", "systemd-resolve")];
        let additional = vec![port("580", "IPv4", "127.0.0.53%lo:53", "systemd-resolve")];

        ListeningPorts::merge(&mut listening_ports, additional);

        assert_eq!(listening_ports.len(), 1);
        assert_eq!(listening_ports[0].name, "127.0.0.53:53");
        assert_eq!(listening_ports[0].pid, "580");
    }

    #[test]
    fn merge_preserves_addresses_with_different_explicit_scopes() {
        let mut listening_ports = vec![port("100", "IPv6", "[fe80::1]%eth0:3000", "server")];
        let additional = vec![port("100", "IPv6", "[fe80::1]%eth1:3000", "server")];

        ListeningPorts::merge(&mut listening_ports, additional);

        assert_eq!(listening_ports.len(), 2);
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
    fn merge_discards_ownerless_supplemental_socket() {
        let mut listening_ports = Vec::new();
        let additional = vec![port("", "IPv6", "*:3000", "")];

        ListeningPorts::merge(&mut listening_ports, additional);

        assert!(listening_ports.is_empty());
    }

    #[test]
    fn socket_names_reconcile_ipv6_scope_placements() {
        assert!(ListeningPorts::same_socket_name(
            "[fe80::1]:3000",
            "[fe80::1]%eth0:3000"
        ));
        assert!(ListeningPorts::same_socket_name(
            "[fe80::1]:3000",
            "[fe80::1%eth0]:3000"
        ));
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

    #[test]
    fn sort_preserves_existing_order_within_pid_and_type() {
        let mut listening_ports = vec![
            port("10", "IPv4", "*:9000", "z-command"),
            port("10", "IPv4", "*:1000", "a-command"),
        ];

        ListeningPorts::sort(&mut listening_ports);

        assert_eq!(listening_ports[0].name, "*:9000");
        assert_eq!(listening_ports[1].name, "*:1000");
    }
}
