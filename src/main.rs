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

#![cfg(not(tarpaulin_include))]

use ports::cmd::{Lsof, Ps};
use ports::ui;
use std::env;
use std::error::Error;
use std::fmt;

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = env::args();
    args.next();

    if let Some(arg) = args.next() {
        return match arg.as_str() {
            "-h" | "--help" => {
                help();
                Ok(())
            }
            "-v" | "--version" => {
                version();
                Ok(())
            }
            "-vv" | "--verbose" => verbose(),
            "-vvv" | "--very-verbose" => very_verbose(),
            arg => {
                eprintln!("Unknown argument: '{arg}'");
                help();
                std::process::exit(2)
            }
        };
    }

    regular()
}

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

fn version() {
    println!("{} {}", env!("CARGO_BIN_NAME"), env!("CARGO_PKG_VERSION"));
}

fn regular() -> Result<(), Box<dyn Error>> {
    let listening_ports = Lsof::listening_ports()?;

    if listening_ports.is_empty() {
        return Ok(());
    }

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

fn verbose() -> Result<(), Box<dyn Error>> {
    let mut listening_ports = Lsof::listening_ports()?;

    if listening_ports.is_empty() {
        return Ok(());
    }

    // Enable more info from `ps aux`.
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

fn very_verbose() -> Result<(), Box<dyn Error>> {
    let mut listening_ports = Lsof::listening_ports()?;

    if listening_ports.is_empty() {
        return Ok(());
    }

    // Enable more info from `ps aux`.
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
