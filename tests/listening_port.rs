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

use std::net::TcpListener;
use std::process::Command;

#[test]
fn lists_a_real_tcp_listener() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("cannot bind test listener");
    let port = listener
        .local_addr()
        .expect("test listener has no local address")
        .port();

    let output = Command::new(env!("CARGO_BIN_EXE_ports"))
        .arg(port.to_string())
        .output()
        .expect("cannot run ports");

    assert!(
        output.status.success(),
        "ports failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains(&format!(":{port}")),
        "ports did not list the test listener:\n{}",
        String::from_utf8_lossy(&output.stdout)
    );
}
