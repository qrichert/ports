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

use std::{fmt, fmt::Write, iter};

const TABLE_COLUMN_SEPARATOR: &str = "  ";

#[derive(Debug, Default)]
pub struct Table<'a> {
    headers: Option<Vec<&'a str>>,
    alignments: Option<&'a [fmt::Alignment]>,
    data: Option<Vec<Vec<&'a str>>>,
}

/// `Table` builder as a blueprint with checks and conversions made.
///
/// `Table` can hold "invalid" state during the build process. Both
/// `headers` and `data` are a required for example, but you can't set
/// both at exactly the same time. And `alignments`, while being
/// required during rendering, can even be omitted in the builder as
/// they have defaults we can use.
///
/// `TableBlueprint` on the other hand, is ready-to-render. All required
/// fields are ensured to be set, and it holds additional context for
/// drawing (e.g., `columns_width`).
struct TableBlueprint<'a> {
    headers: Vec<&'a str>,
    alignments: Vec<fmt::Alignment>,
    data: Vec<Vec<&'a str>>,
    columns_width: Vec<usize>,
}

impl<'a> Table<'a> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            headers: None,
            alignments: None,
            data: None,
        }
    }

    pub fn headers(&mut self, headers: &'a [impl AsRef<str>]) -> &mut Self {
        let headers: Vec<&str> = headers.iter().map(AsRef::as_ref).collect();
        self.headers = Some(headers);
        self
    }

    pub fn alignments(&mut self, alignments: &'a [fmt::Alignment]) -> &mut Self {
        self.alignments = Some(alignments);
        self
    }

    pub fn data(&mut self, data: &'a [Vec<impl AsRef<str>>]) -> &mut Self {
        let data: Vec<Vec<&str>> = data
            .iter()
            .map(|row| row.iter().map(AsRef::as_ref).collect())
            .collect();
        self.data = Some(data);
        self
    }

    fn render(&self) -> String {
        let table = self.make_table_blueprint();

        if table.data.is_empty() {
            return format!("{}\n", table.headers.join("  "));
        }

        let mut output = String::new();

        let mut render_row = |row: &Vec<&str>| {
            for (i, cell) in row.iter().enumerate() {
                let width = table.columns_width[i];
                let alignment = table.alignments[i];

                let is_last_column = i == table.headers.len() - 1;

                let _ = match alignment {
                    fmt::Alignment::Left if is_last_column => write!(output, "{cell}"),
                    fmt::Alignment::Left => write!(output, "{cell:<width$}"),
                    fmt::Alignment::Right => write!(output, "{cell:>width$}"),
                    fmt::Alignment::Center => write!(output, "{cell:^width$}"),
                };

                if is_last_column {
                    output.push('\n');
                } else {
                    output.push_str(TABLE_COLUMN_SEPARATOR);
                }
            }
        };

        // Wrongly marked uncovered.
        #[cfg(not(tarpaulin_include))]
        let rows = iter::once(&table.headers).chain(table.data.iter());

        for row in rows {
            render_row(row);
        }

        output
    }

    fn make_table_blueprint(&self) -> TableBlueprint {
        let headers = self.headers.as_ref().expect("headers are required");
        let alignments = self.get_alignments_or_default();
        let data = self.data.as_ref().expect("data is required");

        Self::ensure_data_consistency(headers, &alignments, data);

        let columns_width = Self::determine_columns_width(headers, data);

        TableBlueprint {
            headers: headers.to_owned(),
            alignments,
            data: data.to_owned(),
            columns_width,
        }
    }

    fn get_alignments_or_default(&self) -> Vec<fmt::Alignment> {
        let nb_headers = self.headers.as_ref().expect("headers are required").len();
        match self.alignments {
            Some(alignments) => alignments.to_vec(),
            None => [fmt::Alignment::Left].repeat(nb_headers),
        }
    }

    /// Ensure data is consistent.
    ///
    /// "Consistent" essentially means the number of headers matches
    /// the number of alignment properties, and the number of columns
    /// in the data.
    fn ensure_data_consistency(
        headers: &[&str],
        alignments: &[fmt::Alignment],
        data: &[Vec<&str>],
    ) {
        assert_eq!(
            headers.len(),
            alignments.len(),
            "number of headers must match alignments"
        );
        assert!(
            data.iter().all(|row| row.len() == headers.len()),
            "number of headers must match columns in data"
        );
    }

    /// Determine the width of each column.
    ///
    /// The width of a column is the number of characters in the longest
    /// value held in the column (including header).
    fn determine_columns_width(headers: &[&str], data: &[Vec<&str>]) -> Vec<usize> {
        let mut cols_width = vec![0; headers.len()];
        for i in 0..headers.len() {
            let column_values: Vec<&str> = data.iter().map(|x| x[i]).collect();
            let max_width = Self::width_of_longest_value_in_column(headers[i], &column_values);
            cols_width[i] = max_width;
        }
        cols_width
    }

    fn width_of_longest_value_in_column(header: &str, column_values: &[&str]) -> usize {
        let header = iter::once(&header);
        let column_values = column_values.iter();

        header
            .chain(column_values)
            .map(|x| x.chars().count())
            .max()
            .expect("iterator cannot be empty because header is required")
    }
}

impl fmt::Display for Table<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let output = self.render();
        write!(f, "{output}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_regular() {
        let table = Table::new()
            .headers(&["SHORT", "WITH SPACE", "LAST COLUMN"])
            .alignments(&[
                fmt::Alignment::Left,
                fmt::Alignment::Left,
                fmt::Alignment::Left,
            ])
            .data(&[
                vec![
                    "Value larger than header",
                    "Column name has space",
                    "No trailing whitespace",
                ],
                vec!["---", "---", "---"],
            ])
            .to_string();

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
    fn table_headers_alignment() {
        let table = Table::new()
            .headers(&["ALIGN-LEFT", "ALIGN-CENTER", "ALIGN-RIGHT"])
            .alignments(&[
                fmt::Alignment::Left,
                fmt::Alignment::Center,
                fmt::Alignment::Right,
            ])
            .data(&[
                vec![
                    "Header is aligned Left",
                    "Header is aligned Center",
                    "Header is aligned Right",
                ],
                vec!["---", "---", "---"],
            ])
            .to_string();

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
    fn table_values_alignment() {
        let table = Table::new()
            .headers(&["ALIGN-LEFT", "ALIGN-CENTER", "ALIGN-RIGHT"])
            .alignments(&[
                fmt::Alignment::Left,
                fmt::Alignment::Center,
                fmt::Alignment::Right,
            ])
            .data(&[vec!["Left", "Center", "Right"], vec!["---", "---", "---"]])
            .to_string();

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
    fn table_default_alignments() {
        let table = Table::new()
            .headers(&["VALUE LEFT", "COLUMN LEFT"])
            .data(&[vec!["---", "----------------"]])
            .to_string();

        println!("{table}");
        assert_eq!(
            table,
            "\
VALUE LEFT  COLUMN LEFT
---         ----------------
"
        );
    }

    #[test]
    fn table_with_empty_data() {
        let table = Table::new()
            .headers(&["SHORT", "WITH SPACE", "LAST COLUMN"])
            .alignments(&[
                fmt::Alignment::Left,
                fmt::Alignment::Left,
                fmt::Alignment::Left,
            ])
            .data(&[] as &[Vec<&str>; 0])
            .to_string();

        println!("{table}");
        assert_eq!(
            table,
            "\
SHORT  WITH SPACE  LAST COLUMN
"
        );
    }

    #[test]
    fn table_completely_empty() {
        let table = Table::new()
            .headers(&[] as &[&str; 0])
            .alignments(&[])
            .data(&[] as &[Vec<&str>; 0])
            .to_string();

        println!("{table}");
        assert_eq!(table, "\n");
    }

    #[test]
    fn table_completely_with_default_alignments() {
        let table = Table::new()
            .headers(&[] as &[&str; 0])
            .data(&[] as &[Vec<&str>; 0])
            .to_string();

        println!("{table}");
        assert_eq!(table, "\n");
    }

    #[test]
    #[should_panic(expected = "number of headers must match alignments")]
    fn table_nb_headers_neq_nb_alignments() {
        Table::new()
            .headers(&["COLUMN 1", "COLUMN 2"])
            .alignments(&[
                fmt::Alignment::Left,
                fmt::Alignment::Left,
                fmt::Alignment::Left,
            ])
            .data(&[vec!["---", "---"]])
            .to_string();
    }

    #[test]
    #[should_panic(expected = "number of headers must match columns in data")]
    fn table_nb_headers_neq_nb_columns_in_data() {
        Table::new()
            .headers(&["COLUMN 1", "COLUMN 2"])
            .alignments(&[fmt::Alignment::Left, fmt::Alignment::Left])
            .data(&[
                vec!["---", "---"],
                vec!["---", "---", "---"],
                vec!["---", "---"],
            ])
            .to_string();
    }
}
