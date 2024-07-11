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

use ports::cmd::Lsof;
use std::error::Error;
use std::{fmt, fmt::Write};

fn main() -> Result<(), Box<dyn Error>> {
    let listening_ports = Lsof::listening_ports()?;

    if listening_ports.is_empty() {
        return Ok(());
    }
    // TODO: Enable more info from `ps aux`.
    //   let ps = cmd::Ps::running_processes();

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
        to_table(
            &["COMMAND", "PID", "USER", "TYPE", "NODE", "HOST:PORT"],
            &[
                fmt::Alignment::Left,
                fmt::Alignment::Right,
                fmt::Alignment::Left,
                fmt::Alignment::Left,
                fmt::Alignment::Left,
                fmt::Alignment::Right
            ],
            &listening_ports
        )
    );

    Ok(())
}

fn to_table(headers: &[&str], alignments: &[fmt::Alignment], data: &[Vec<&String>]) -> String {
    const COLUMN_SEPARATOR: &str = "  ";

    fn column_width(header: &str, column_values: &[&String]) -> usize {
        std::cmp::max(
            header.chars().count(),
            column_values
                .iter()
                .map(|x| x.chars().count())
                .max()
                .unwrap(),
        )
    }

    if data.is_empty() {
        return headers.join("  ");
    }
    assert_eq!(headers.len(), alignments.len());
    assert_eq!(headers.len(), data[0].len());

    // Normalize to strings. It's better to normalize the header to
    // Strings instead of the data to references. There are probably
    // many more rows than there are columns.
    let headers: Vec<String> = headers.iter().map(|x| String::from(*x)).collect();

    // Determine the width of each column.
    let mut cols_width = vec![0; headers.len()];
    for i in 0..headers.len() {
        let values: Vec<&String> = data.iter().map(|x| x[i]).collect();
        let width = column_width(&headers[i], &values);
        cols_width[i] = width;
    }

    let mut table = String::new();

    let mut render_row = |row: &Vec<&String>| {
        for i in 0..headers.len() {
            let cell = row[i];
            let width = cols_width[i];
            let alignment = alignments[i];

            let is_last_column = i == headers.len() - 1;

            let _ = match alignment {
                fmt::Alignment::Left if is_last_column => write!(table, "{cell}"),
                fmt::Alignment::Left => write!(table, "{cell:<width$}"),
                fmt::Alignment::Right => write!(table, "{cell:>width$}"),
                fmt::Alignment::Center => write!(table, "{cell:^width$}"),
            };

            if is_last_column {
                table.push('\n');
            } else {
                table.push_str(COLUMN_SEPARATOR);
            }
        }
    };

    render_row(&headers.iter().collect());
    for row in data {
        render_row(row);
    }

    table
}
