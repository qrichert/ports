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

use std::error::Error;
use std::fmt;
use std::process::{Command, Output};
use std::str::Lines;

#[derive(Eq, PartialEq)]
pub struct PsError {
    reason: &'static str,
}

impl Error for PsError {}

impl fmt::Debug for PsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.reason)
    }
}

impl fmt::Display for PsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
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
    _cannot_instantiate: std::marker::PhantomData<()>,
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

pub struct Ps;

impl Ps {
    /// Use `ps` to get process info.
    ///
    /// # Errors
    ///
    /// Errors if the `ps` executable is not found, or if the command
    ///  exits with a non-zero exit code.
    pub fn processes_info(pids: &[&String]) -> Result<Vec<ProcessInfo>, PsError> {
        let output = Self::ps()?;
        let mut output = output.lines();

        let header_columns = Self::extract_header_columns(&mut output)?;
        let detail_lines = Self::extract_detail_lines_of_processes(&mut output);

        let pinfo = Self::map_detail_values_to_properties(&header_columns, &detail_lines);
        let pinfo = Self::keep_only_relevant_pids(pinfo, pids);

        Ok(pinfo)
    }

    #[cfg(not(tarpaulin_include))]
    fn ps() -> Result<String, PsError> {
        #![allow(unreachable_code)]
        #[cfg(test)]
        {
            let fixture =
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ps.txt");
            let output = std::fs::read_to_string(fixture).expect("cannot read test fixture");
            return Ok(output);
        }

        let output = Command::new("ps").arg("aux").output();

        match output {
            Ok(output) => Self::handle_output_ok(&output),
            Err(_) => Self::handle_output_err(),
        }
    }

    fn handle_output_ok(output: &Output) -> Result<String, PsError> {
        if output.status.success() {
            // Exit 0.
            Ok(String::from_utf8_lossy(&output.stdout).into_owned())
        } else {
            // Non-zero exit code.
            Err(PsError {
                reason: "The ps command has failed in an unexpected way.",
            })
        }
    }

    fn handle_output_err() -> Result<String, PsError> {
        Err(PsError {
            reason: "Unable to locate the ps executable on the system.",
        })
    }

    /// Extract first line as column titles.
    fn extract_header_columns(output: &mut Lines) -> Result<Vec<String>, PsError> {
        let Some(header) = output.next() else {
            return Err(PsError {
                reason: "The ps output is missing the header.",
            });
        };
        let header = header.to_ascii_uppercase(); // To make sure.
        let header: Vec<&str> = header.split_ascii_whitespace().collect();

        let header = Self::normalize_header_columns(&header);

        if !Self::header_contains_all_properties(&header) {
            return Err(PsError {
                reason: "The ps output is missing expected properties.",
            });
        }

        Ok(header.iter().map(ToString::to_string).collect())
    }

    fn normalize_header_columns<'a>(header: &[&'a str]) -> Vec<&'a str> {
        header
            .iter()
            .map(|col| match col {
                // 'START' may be called 'STARTED' in certain versions.
                &"STARTED" => "START",
                _ => col,
            })
            .collect()
    }

    fn header_contains_all_properties(header: &[&str]) -> bool {
        for col in Self::headers() {
            if !header.contains(col) {
                return false;
            }
        }
        true
    }

    fn headers() -> &'static [&'static str] {
        &["USER", "PID", "%CPU", "%MEM", "START", "TIME", "COMMAND"]
    }

    /// Extract the rest of the output as detail lines.
    fn extract_detail_lines_of_processes<'a>(output: &'a mut Lines) -> Vec<Vec<&'a str>> {
        output
            .map(|line| line.split_ascii_whitespace().collect())
            .collect()
    }

    /// Associate column values to values properties.
    fn map_detail_values_to_properties(
        header_columns: &[String],
        detail_lines: &[Vec<&str>],
    ) -> Vec<ProcessInfo> {
        if detail_lines.is_empty() {
            return Vec::new();
        }

        let mut ps = Vec::with_capacity(detail_lines.len());

        // Each line is a `Vec` of columns (split on whitespace).
        for detail_line in detail_lines {
            let mut process = ProcessInfo::new();

            for col in 0..header_columns.len() {
                let value = String::from(detail_line[col]);

                match header_columns[col].as_str() {
                    "USER" => process.user = value,
                    "PID" => process.pid = value,
                    "%CPU" => process.pc_cpu = value,
                    "%MEM" => process.pc_mem = value,
                    "START" => process.start = value,
                    "TIME" => process.time = value,
                    "COMMAND" => {
                        // 'COMMAND' is the last column, and its values
                        // may contain spaces (e.g, `python3 -m http.server`).
                        // So, we just "eat" the columns to the end.
                        // Note: This has the side-effect of compressing
                        // multiple spaces into one. Not ideal, but we
                        // can argue it's a feature, not a shortcoming.
                        let remaining = detail_line[col..].join(" ");
                        process.command = remaining;
                    }
                    _ => continue,
                };
            }

            ps.push(process);
        }

        ps
    }

    fn keep_only_relevant_pids(pinfo: Vec<ProcessInfo>, pids: &[&String]) -> Vec<ProcessInfo> {
        pinfo
            .into_iter()
            .filter(|process| pids.contains(&&process.pid))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::process::ExitStatusExt;
    use std::process::ExitStatus;

    fn new_pinfo_with_pid(pid: &str) -> ProcessInfo {
        let mut pinfo = ProcessInfo::new();
        pinfo.pid = pid.to_string();
        pinfo
    }

    #[test]
    fn pserror_debug() {
        let error = PsError {
            reason: "an error has occurred",
        };

        assert_eq!(format!("{error:?}"), "an error has occurred");
    }

    #[test]
    fn pserror_display() {
        let error = PsError {
            reason: "an error has occurred",
        };

        assert_eq!(error.to_string(), "an error has occurred");
    }

    #[test]
    fn ps_successful_read() {
        let output = Output {
            status: ExitStatus::from_raw(0),
            stdout: b"<stdout>".to_vec(),
            stderr: b"<stderr>".to_vec(),
        };

        let res = Ps::handle_output_ok(&output).unwrap();

        assert_eq!(res, "<stdout>");
    }

    #[test]
    fn ps_unsuccessful_read() {
        let output = Output {
            status: ExitStatus::from_raw(1),
            stdout: b"<stdout>".to_vec(),
            stderr: b"<stderr>".to_vec(),
        };

        let res = Ps::handle_output_ok(&output).unwrap_err();

        assert_eq!(
            res,
            PsError {
                reason: "The ps command has failed in an unexpected way.",
            }
        );
    }

    #[test]
    fn ps_error_with_command() {
        let res = Ps::handle_output_err().unwrap_err();

        assert_eq!(
            res,
            PsError {
                reason: "Unable to locate the ps executable on the system.",
            }
        );
    }

    #[test]
    fn processinfo_default() {
        assert_eq!(ProcessInfo::new(), ProcessInfo::default());
    }

    #[test]
    fn processinfo_new() {
        let process = ProcessInfo::new();

        assert_eq!(
            process,
            ProcessInfo {
                user: String::new(),
                pid: String::new(),
                pc_cpu: String::new(),
                pc_mem: String::new(),
                start: String::new(),
                time: String::new(),
                command: String::new(),
                _cannot_instantiate: std::marker::PhantomData,
            }
        );
    }

    // The `Ps::processes_info()` should be integration tests. But at
    // this scale, it's easier like this.

    #[test]
    fn processes_info() {
        let processes_info = Ps::processes_info(&[&String::from("2673")]).unwrap();

        let process: ProcessInfo = processes_info
            .into_iter()
            .find(|x| x.pid == "2673")
            .unwrap();

        assert_eq!(
            process,
            ProcessInfo {
                user: String::from("root"),
                pid: String::from("2673"),
                pc_cpu: String::from("0.0"),
                pc_mem: String::from("0.0"),
                start: String::from("09:27"),
                time: String::from("0:02"),
                command: String::from("/usr/bin/docker-proxy -proto tcp -host-ip 0.0.0.0 -host-port 333 -container-ip 172.19.0.4 -container-port 22"),
                _cannot_instantiate: std::marker::PhantomData,
            }
        );
    }

    #[test]
    fn processes_info_where_command_has_no_spaces() {
        let processes_info = Ps::processes_info(&[&String::from("874")]).unwrap();

        let process: ProcessInfo = processes_info.into_iter().find(|x| x.pid == "874").unwrap();

        assert_eq!(
            process,
            ProcessInfo {
                user: String::from("colord"),
                pid: String::from("874"),
                pc_cpu: String::from("0.0"),
                pc_mem: String::from("0.1"),
                start: String::from("09:27"),
                time: String::from("0:00"),
                command: String::from("/usr/libexec/colord"),
                _cannot_instantiate: std::marker::PhantomData,
            }
        );
    }

    #[test]
    fn extract_header_columns_regular() {
        let headers = Ps::headers().join(" ");
        let output = format!("{headers}\n");
        let mut output = output.lines();

        let columns = Ps::extract_header_columns(&mut output).unwrap();

        assert_eq!(columns, Ps::headers());
    }

    #[test]
    fn extract_header_columns_error_empty_output() {
        let output = String::new();
        let mut output = output.lines();

        let error = Ps::extract_header_columns(&mut output).unwrap_err();

        assert_eq!(
            error,
            PsError {
                reason: "The ps output is missing the header."
            }
        );
    }

    #[test]
    fn extract_header_columns_no_newline_after_only_headers() {
        let headers = Ps::headers().join(" ");
        let mut output = headers.lines();

        let columns = Ps::extract_header_columns(&mut output).unwrap();

        assert_eq!(columns, Ps::headers());
    }

    #[test]
    fn extract_header_columns_error_no_header() {
        let output = String::from("\n");
        let mut output = output.lines();

        let error = Ps::extract_header_columns(&mut output).unwrap_err();

        assert_eq!(
            error,
            PsError {
                // This is considered an empty header line, and so falls
                // into this error, instead of "no header"
                reason: "The ps output is missing expected properties."
            }
        );
    }

    #[test]
    fn extract_header_columns_with_additional_headers() {
        let mut headers = Ps::headers().to_vec();
        headers.push("FOO");
        headers.push("BAR");
        headers.push("BAZ");

        let output = format!("{}\n", headers.join(" "));
        let mut output = output.lines();

        let columns = Ps::extract_header_columns(&mut output).unwrap();

        assert_eq!(columns, headers);
    }

    #[test]
    fn extract_header_columns_error_with_missing_headers() {
        let mut headers = Ps::headers().to_vec();
        headers.pop();

        let output = format!("{}\n", headers.join(" "));
        let mut output = output.lines();

        let error = Ps::extract_header_columns(&mut output).unwrap_err();

        assert_eq!(
            error,
            PsError {
                reason: "The ps output is missing expected properties.",
            }
        );
    }

    #[test]
    fn extract_header_columns_wrong_character_case() {
        let headers = Ps::headers().join(" ").to_lowercase();
        let mut output = headers.lines();

        let columns = Ps::extract_header_columns(&mut output).unwrap();

        assert_eq!(columns, Ps::headers());
    }

    #[test]
    fn extract_header_columns_alternative_names() {
        let headers = Ps::headers().join(" ").replace("START", "STARTED");

        let mut output = headers.lines();

        let columns = Ps::extract_header_columns(&mut output).unwrap();

        assert_eq!(columns, Ps::headers());
    }

    #[test]
    fn extract_detail_lines_of_processes_regular() {
        let output = "\
This is included
This is included too
";
        let mut output = output.lines();

        let detail_lines = Ps::extract_detail_lines_of_processes(&mut output);

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
            String::from("USER"),
            String::from("PID"),
            String::from("%CPU"),
            String::from("%MEM"),
            String::from("START"),
            String::from("TIME"),
            String::from("COMMAND"),
        ];

        let detail_lines = [vec![
            "<user>",
            "<pid>",
            "<pc_cpu>",
            "<pc_mem>",
            "<start>",
            "<time>",
            "<command that started the process>",
        ]];

        let ps = Ps::map_detail_values_to_properties(&header_columns, &detail_lines);

        assert_eq!(
            ps,
            vec![ProcessInfo {
                user: String::from("<user>"),
                pid: String::from("<pid>"),
                pc_cpu: String::from("<pc_cpu>"),
                pc_mem: String::from("<pc_mem>"),
                start: String::from("<start>"),
                time: String::from("<time>"),
                command: String::from("<command that started the process>"),
                _cannot_instantiate: std::marker::PhantomData
            }],
        );
    }

    #[test]
    fn map_detail_values_to_properties_no_detail_lines() {
        let header_columns = [
            String::from("USER"),
            String::from("PID"),
            String::from("%CPU"),
            String::from("%MEM"),
            String::from("START"),
            String::from("TIME"),
            String::from("COMMAND"),
        ];

        let detail_lines = [];

        let ps = Ps::map_detail_values_to_properties(&header_columns, &detail_lines);

        assert_eq!(ps, vec![],);
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

        let ps = Ps::map_detail_values_to_properties(&header_columns, &detail_lines);

        assert_eq!(
            ps,
            vec![ProcessInfo {
                user: String::new(),
                pid: String::from("<pid>"),
                pc_cpu: String::new(),
                pc_mem: String::new(),
                start: String::new(),
                time: String::new(),
                command: String::new(),
                _cannot_instantiate: std::marker::PhantomData
            }],
        );
    }

    #[test]
    fn keep_only_relevant_pids() {
        let processes = vec![
            new_pinfo_with_pid("1"),
            new_pinfo_with_pid("2"),
            new_pinfo_with_pid("3"),
        ];

        let processes =
            Ps::keep_only_relevant_pids(processes, &[&String::from("1"), &String::from("3")]);

        assert_eq!(processes.len(), 2);
        assert_eq!(processes[0].pid, "1");
        assert_eq!(processes[1].pid, "3");
    }
}
