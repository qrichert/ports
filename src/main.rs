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

fn to_table(
    headers: &[impl AsRef<str>],
    alignments: &[fmt::Alignment],
    data: &[Vec<impl AsRef<str>>],
) -> String {
    const COLUMN_SEPARATOR: &str = "  ";

    fn column_width(header: &str, column_values: &[&str]) -> usize {
        std::cmp::max(
            header.chars().count(),
            column_values
                .iter()
                .map(|x| x.chars().count())
                .max()
                .unwrap(),
        )
    }

    let headers: Vec<&str> = headers.iter().map(AsRef::as_ref).collect();
    let data: Vec<Vec<&str>> = data
        .iter()
        .map(|row| row.iter().map(AsRef::as_ref).collect())
        .collect();

    if data.is_empty() {
        return format!("{}\n", headers.join("  "));
    }
    assert_eq!(
        headers.len(),
        alignments.len(),
        "number of headers must match alignments"
    );
    assert!(
        data.iter().all(|row| row.len() == headers.len()),
        "number of headers must match columns in data"
    );

    // Determine the width of each column.
    let mut cols_width = vec![0; headers.len()];
    for i in 0..headers.len() {
        let values: Vec<&str> = data.iter().map(|x| x[i]).collect();
        let width = column_width(headers[i], &values);
        cols_width[i] = width;
    }

    let mut table = String::new();

    let mut render_row = |row: &Vec<&str>| {
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

    render_row(&headers);
    for row in data {
        render_row(&row);
    }

    table
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_table_regular() {
        let table = to_table(
            &["SHORT", "WITH SPACE", "LAST COLUMN"],
            &[
                fmt::Alignment::Left,
                fmt::Alignment::Left,
                fmt::Alignment::Left,
            ],
            &[
                vec![
                    "Value larger than header",
                    "Column name has space",
                    "No trailing whitespace",
                ],
                vec!["---", "---", "---"],
            ],
        );

        println!("{table}");
        assert_eq!(
            table,
            "\
SHORT                     WITH SPACE             LAST COLUMN
Value larger than header  Column name has space  No trailing whitespace
---                       ---                    ---
"
        );
    }

    #[test]
    fn to_table_headers_alignment() {
        let table = to_table(
            &["ALIGN-LEFT", "ALIGN-CENTER", "ALIGN-RIGHT"],
            &[
                fmt::Alignment::Left,
                fmt::Alignment::Center,
                fmt::Alignment::Right,
            ],
            &[
                vec![
                    "Header is aligned Left",
                    "Header is aligned Center",
                    "Header is aligned Right",
                ],
                vec!["---", "---", "---"],
            ],
        );

        println!("{table}");
        assert_eq!(
            table,
            "\
ALIGN-LEFT                    ALIGN-CENTER                    ALIGN-RIGHT
Header is aligned Left  Header is aligned Center  Header is aligned Right
---                               ---                                 ---
"
        );
    }

    #[test]
    fn to_table_values_alignment() {
        let table = to_table(
            &["ALIGN-LEFT", "ALIGN-CENTER", "ALIGN-RIGHT"],
            &[
                fmt::Alignment::Left,
                fmt::Alignment::Center,
                fmt::Alignment::Right,
            ],
            &[vec!["Left", "Center", "Right"], vec!["---", "---", "---"]],
        );

        println!("{table}");
        assert_eq!(
            table,
            "\
ALIGN-LEFT  ALIGN-CENTER  ALIGN-RIGHT
Left           Center           Right
---             ---               ---
"
        );
    }

    #[test]
    fn to_table_empty() {
        let table = to_table(
            &["SHORT", "WITH SPACE", "LAST COLUMN"],
            &[
                fmt::Alignment::Left,
                fmt::Alignment::Left,
                fmt::Alignment::Left,
            ],
            &[] as &[Vec<&str>; 0],
        );

        println!("{table}");
        assert_eq!(
            table,
            "\
SHORT  WITH SPACE  LAST COLUMN
"
        );
    }

    #[test]
    #[should_panic(expected = "number of headers must match alignments")]
    fn to_table_nb_headers_neq_nb_alignments() {
        to_table(
            &["COLUMN 1", "COLUMN 2"],
            &[
                fmt::Alignment::Left,
                fmt::Alignment::Left,
                fmt::Alignment::Left,
            ],
            &[vec!["---", "---"]],
        );
    }

    #[test]
    #[should_panic(expected = "number of headers must match columns in data")]
    fn to_table_nb_headers_neq_nb_columns_in_data() {
        to_table(
            &["COLUMN 1", "COLUMN 2"],
            &[fmt::Alignment::Left, fmt::Alignment::Left],
            &[
                vec!["---", "---"],
                vec!["---", "---", "---"],
                vec!["---", "---"],
            ],
        );
    }
}
