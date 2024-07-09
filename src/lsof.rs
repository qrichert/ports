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

#[allow(clippy::module_name_repetitions)]
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

pub struct Lsof;

impl Lsof {
    /// Use `lsof` to list listening ports.
    ///
    /// # Errors
    ///
    /// Errors if the `lsof` executable is not found, or if the command
    /// exits with a non-zero exit code.
    pub fn listening_ports() -> Result<String, LsofError> {
        //let status = Command::new("lsof").arg("-i", "-P", "-n"); // TODO: Manual grep
        // -i List IP sockets.
        // -P Do not resolve port names (list port number instead of its name).
        // -n Do not resolve hostnames (no DNS).
        let output = Command::new("/bin/bash")
            .arg("-c")
            .arg("lsof -i -n -P | grep --color=never LISTEN")
            .output();

        match output {
            Ok(output) => Self::handle_output_ok(&output),
            Err(_) => Self::handle_output_err(),
        }
    }

    fn handle_output_ok(output: &Output) -> Result<String, LsofError> {
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).into_owned())
        } else {
            // Non-zero exit code.
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
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
