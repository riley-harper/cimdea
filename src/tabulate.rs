//! The high level module for executing and formatting tabulations.
//!
//!   The result of the tabulations are tabulation::Table structs that
//! carry some metadata information with them to be used by formatters or even codebook
//! generators.
//!
use std::str::FromStr;

use crate::conventions::Context;
use crate::ipums_metadata_model::IpumsDataType;
use crate::mderror::{metadata_error, MdError};
use crate::query_gen::tab_queries;
use crate::query_gen::DataPlatform;
use crate::request::DataRequest;
use crate::request::InputType;
use crate::request::RequestVariable;

use duckdb::Connection;
use serde::ser::Error;
use serde::Serialize;

const DEBUG: bool = false;

/// The format of an output table.
#[derive(Clone, Copy, Debug)]
pub enum TableFormat {
    Csv,
    Html,
    Json,
    TextTable,
}

impl FromStr for TableFormat {
    type Err = MdError;

    /// Parse a `TableFormat` from an `&str`.
    ///
    /// The parsing is case-insensitive and accepts the strings "csv", "json", "text", and "html".
    ///
    /// ```
    /// use cimdea::tabulate::TableFormat;
    /// use std::str::FromStr;
    ///
    /// let format = TableFormat::from_str("text").unwrap();
    /// assert!(matches!(format, TableFormat::TextTable));
    /// ```
    fn from_str(name: &str) -> Result<Self, Self::Err> {
        let tf = match name.to_ascii_lowercase().as_str() {
            "csv" => Self::Csv,
            "json" => Self::Json,
            "text" => Self::TextTable,
            "html" => Self::Html,
            _ => return Err(MdError::Msg("unknown format name.".to_string())),
        };
        Ok(tf)
    }
}

#[derive(Clone, Debug)]
pub enum OutputColumn {
    Constructed {
        name: String,
        width: usize,
        data_type: IpumsDataType,
    },
    RequestVar(RequestVariable),
}

/// The RequestVar variant on OutputColumn has a real RequestVariable struct because there is a lot of useful information in there
/// to help format or generate codebooks etc. However for basic table serialization we only want to capture the
/// name, type and format width.We don't want to serialize the whole content of the RequestVar varient into JSON.
/// This serialization exists to convert an tabulate::Table into JSON for outside consumption.
impl Serialize for OutputColumn {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStructVariant;
        match &self {
            Self::Constructed {
                name,
                width,
                data_type,
            } => {
                let mut ser =
                    serializer.serialize_struct_variant("OutputColumn", 0, "Constructed", 3)?;
                ser.serialize_field("name", &name)?;
                ser.serialize_field("width", &width)?;
                ser.serialize_field("data_type", &format!("{}", data_type))?;
                ser.end()
            }
            Self::RequestVar(ref v) => {
                let mut ser =
                    serializer.serialize_struct_variant("OutputColumn", 1, "RequestVar", 3)?;
                let width = v.requested_width().map_err(S::Error::custom)?;
                let data_type = match v.variable.data_type {
                    Some(ref data_type) => data_type.to_string(),
                    None => {
                        let err = metadata_error!("missing data type for variable {}", v.name);
                        return Err(S::Error::custom(err));
                    }
                };

                ser.serialize_field("name", &v.name)?;
                ser.serialize_field("width", &width)?;
                ser.serialize_field("data_type", &data_type)?;
                ser.end()
            }
        }
    } // serialize trait
} // impl

impl OutputColumn {
    pub fn name(&self) -> String {
        match self {
            Self::Constructed { ref name, .. } => name.clone(),
            Self::RequestVar(ref v) => v.name.clone(),
        }
    }

    pub fn width(&self) -> Result<usize, MdError> {
        match self {
            Self::Constructed { ref width, .. } => Ok(*width),
            Self::RequestVar(ref v) => {
                if !v.is_general() {
                    if let Some((_, wid)) = v.variable.formatting {
                        Ok(wid)
                    } else {
                        Err(metadata_error!("width from metadata variable required"))
                    }
                } else {
                    if let Some(general_width) = v.variable.general_width {
                        Ok(general_width)
                    } else {
                        Err(MdError::Msg(format!(
                            "cannot determine general width for variable {}",
                            self.name()
                        )))
                    }
                }
            }
        }
    }
} // impl

// If we want we can use the IpumsVariable categories to replace the numbers in the results (rows)
// with category labels and use the data type and width information to better format the table.

#[derive(Clone, Debug, Serialize)]
pub struct Table {
    pub heading: Vec<OutputColumn>, // variable name columns
    pub rows: Vec<Vec<String>>,
}

impl Table {
    pub fn format_as_text(&self) -> Result<String, MdError> {
        let mut out = String::new();
        let widths = self.column_widths()?;
        for (column, _v) in self.heading.iter().enumerate() {
            let name = self.heading[column].name();
            let column_header = format!("| {n:>w$} ", n = &name, w = widths[column]);
            out.push_str(&column_header);
        }
        out.push_str("|\n");
        out.push_str(&format!(
            "|{:}|",
            str::repeat(&"-", self.text_table_width()? - 2)
        ));
        out.push_str("\n");

        for r in &self.rows {
            for (column, item) in r.iter().enumerate() {
                let w = widths[column];
                let formatted_item = format!("| {value:>width$} ", value = &item, width = w);
                out.push_str(&formatted_item);
            }
            out.push_str("|\n");
        }
        Ok(out)
    }

    pub fn text_table_width(&self) -> Result<usize, MdError> {
        Ok(1 + 3 * self.heading.len() + self.column_widths()?.iter().sum::<usize>())
    }

    fn column_widths(&self) -> Result<Vec<usize>, MdError> {
        let mut widths = Vec::new();
        for (_column, var) in self.heading.iter().enumerate() {
            let name_width = var.name().len();
            let width = var.width()?;
            if name_width < width {
                widths.push(width);
            } else {
                widths.push(name_width);
            }
            /*
            else  if let Some(w) = self.width_from_data(column) {
                    if name_width < w {
                        widths.push(w);
                    } else {
                        widths.push(name_width);
                    }
                } else {
                    return Err(MdError::Msg("Can't determine column width of data.".to_string()));
                }
            }
            */
        }
        Ok(widths)
    }

    #[allow(unused)]
    fn width_from_data(&self, column: usize) -> Option<usize> {
        self.rows.iter().map(|r| r[column].len()).max()
    }

    pub fn empty() -> Self {
        Self {
            rows: Vec::new(),
            heading: Vec::new(),
        }
    }
}

#[derive(Debug)]
pub struct Tabulation(pub Vec<Table>);

impl Tabulation {
    pub fn output(&self, format: TableFormat) -> Result<String, MdError> {
        let output = match format {
            TableFormat::Html | TableFormat::Csv => {
                todo!("Output format {:?} not implemented yet.", format)
            }
            TableFormat::Json => match serde_json::to_string_pretty(&self.0) {
                Ok(output) => output,
                Err(err) => {
                    return Err(MdError::Msg(format!(
                        "Cannot serialize result into json: {err}"
                    )));
                }
            },
            TableFormat::TextTable => {
                let mut output = String::new();
                for table in &self.0 {
                    let table_text = table.format_as_text()?;
                    output.push_str(&format!("{table_text}\n"));
                }
                output
            }
        };

        Ok(output)
    }

    pub fn into_inner(self) -> Vec<Table> {
        self.0
    }
}

/// Compute the result of a tabulation request.
///
/// A single request can result in multiple tables. Normally there is one table per IPUMS dataset
/// in the request. Right now `InputType::Parquet` and `DataPlatform::Duckdb` are hard-coded in;
/// they're the main use-case for now. `InputType::Csv` ought to be pretty interchangable except
/// for performance implications. The `DataPlatform::DataFusion` alternative would require minor
/// additions to the query generation module. `DataPlatform::Polars` is also planned and shouldn't
/// require too many additional query gen updates, but it is unimplemented for now.
pub fn tabulate<R>(ctx: &Context, rq: R) -> Result<Tabulation, MdError>
where
    R: DataRequest,
{
    let requested_output_columns = rq
        .get_request_variables()
        .iter()
        .map(|v| OutputColumn::RequestVar(v.clone()))
        .collect::<Vec<OutputColumn>>();

    let mut tables: Vec<Table> = Vec::new();
    let sql_queries = tab_queries(ctx, rq, &InputType::Parquet, &DataPlatform::Duckdb)?;
    let conn = Connection::open_in_memory()?;
    for q in sql_queries {
        if DEBUG {
            println!("{}", &q);
        }
        let mut stmt = conn.prepare(&q)?;
        let mut rows = stmt.query([])?;

        let mut output = Table {
            heading: Vec::new(),
            rows: Vec::new(),
        };
        output.heading.push(OutputColumn::Constructed {
            name: "ct".to_string(),
            width: 10,
            data_type: IpumsDataType::Integer,
        });
        output.heading.push(OutputColumn::Constructed {
            name: "weighted_ct".to_string(),
            width: 10,
            data_type: IpumsDataType::Integer,
        });
        output.heading.extend(requested_output_columns.clone());

        while let Some(row) = rows.next()? {
            let mut this_row = Vec::new();
            // Must do this here on row rather than getting column_names() from
            // stmt.column_names() because of a bug in the DuckDB API -- it
            // works on rsqlite but not DuckDB.
            // See https://github.com/duckdb/duckdb-rs/issues/251
            let column_names = row.as_ref().column_names();
            for (column_number, column_name) in column_names.iter().enumerate() {
                /*
                // Leaving this here as a reminder of how to debug the DuckDB result
                // set values; it's different than Rqlite.
                match row.get_ref(column_number) {
                    Ok(d) =>println!("{}: {:?}", &column_name, &d),
                    Err(e) => println!("{}: error: {}", &column_name, e),

                }
                */
                let item: isize = match row.get(column_number) {
                    Ok(i) => i,
                    Err(e) => {
                        return Err(MdError::Msg(format!(
                            "Can't extract value for '{}', error was '{}'",
                            &column_name, e
                        )))
                    }
                };
                this_row.push(format!("{}", item));
            }
            output.rows.push(this_row);
        }
        tables.push(output);
    }

    Ok(Tabulation(tables))
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::request::{AbacusRequest, SimpleRequest};
    use std::time::*;

    #[test]
    fn test_complex_tabulation() {
        let tabtime = Instant::now();
        let json_request = include_str!("../tests/requests/incwage_marst_example.json");

        let (ctx, rq) = AbacusRequest::try_from_json(&json_request)
            .expect("Error loading test context and deserializing test request.");

        //println!("Codebook: {}", rq.print_codebook());

        let result = tabulate(&ctx, rq);
        if let Err(ref e) = result {
            eprintln!("Error setting up test: {:?}", e);
        }
        println!("Test tabulation took {} ms", tabtime.elapsed().as_millis());
        assert!(result.is_ok());
        if let Ok(tab) = result {
            if let Ok(output) = tab.output(TableFormat::TextTable) {
                println!("{output}");
            }

            let tables = tab.into_inner();
            assert_eq!(2, tables.len());

            for (index, table) in tables.iter().enumerate() {
                // There are some category combinations  rare enough not to exist on every sample
                if index == 0 {
                    assert_eq!(
                        79,
                        table.rows.len(),
                        "6 marst X 15 incwage - a few combinations"
                    );
                }
                if index == 1 {
                    assert_eq!(
                        77,
                        table.rows.len(),
                        "6 marst X 15 incwage - a few combinations"
                    );
                }
            }
        }
    }

    #[test]
    fn test_subpopulation() {
        let json_request =
            include_str!("../tests/requests/single_condition_subpop_abacus_request.json");

        let (ctx, rq) = AbacusRequest::try_from_json(json_request)
            .expect("Error loading test context and deserializing test request.");

        let result = tabulate(&ctx, rq);
        if let Err(ref e) = result {
            eprintln!("Error setting up test: {:?}", e);
        }

        assert!(result.is_ok());
        if let Ok(tab) = result {
            if let Ok(output) = tab.output(TableFormat::TextTable) {
                println!("{output}");
            }

            let tables = tab.into_inner();
            for table in tables {
                // Three categories of SCHOOL
                assert_eq!(3, table.rows.len(), "Three SCHOOL categories");
            }
        }
    }

    #[test]
    fn test_sample_line_weights() {
        let data_root = String::from("tests/data_root");
        let (ctx, rq) = SimpleRequest::from_names(
            "usa",
            &["us1940a"],
            &["VETSTAT"],
            Some("P".to_string()),
            None,
            Some(data_root),
        )
        .expect(
            "Setting up this request and context is for a subsequent test and should always work.",
        );

        let result = tabulate(&ctx, rq);
        if let Err(ref e) = result {
            println!("{}", e);
        }

        assert!(result.is_ok(), "Should have tabulated.");
        if let Ok(tab) = result {
            let tables = tab.into_inner();
            assert_eq!(1, tables.len());
            for t in tables {
                println!(
                    "{}",
                    t.format_as_text()
                        .expect("should be able to format as text")
                );
                assert_eq!(4, t.rows.len());
                assert_eq!(3, t.rows[0].len());

                // The counts of rows should add up to all the rows
                // in the test person file, about 1% of the 1% sample.
                // Previously I had '98' because I was wrongly applying
                // the SELFWTSL == 2 filter before counting.
                assert_eq!(
                    "1118", t.rows[0][0],
                    "Should be the number of person records in the data for this category."
                );

                assert_eq!("76100", t.rows[0][1], "98 should get weighted to 42300");
            }
        }
    }

    #[test]
    fn test_hh_only() {
        let data_root = String::from("tests/data_root");
        let (ctx, rq) = SimpleRequest::from_names(
            "usa",
            &["us2015b"],
            &["GQ", "STATEFIP"],
            Some("P".to_string()),
            None,
            Some(data_root),
        )
        .expect(
            "Setting up this request and context is for a subsequent test and should always work.",
        );

        println!("Tab with only hh vars:");

        let result = tabulate(&ctx, rq);
        if let Err(ref e) = result {
            println!("{}", e);
        }

        assert!(result.is_ok(), "Should have tabulated.");
        if let Ok(tab) = result {
            let tables = tab.into_inner();
            assert_eq!(1, tables.len());
            for t in tables {
                println!(
                    "{}",
                    t.format_as_text()
                        .expect("should be able to format as text")
                );
                assert_eq!(4, t.rows.len());
                assert_eq!(4, t.rows[0].len());
                // The unweighted count of people in GQ == 1 in PR
                assert_eq!(
                    "29846", t.rows[0][0],
                    "Should be the number of person records in the data, not number of households."
                );

                // The STATEFIP code should be 72 for PR
                assert_eq!("72", t.rows[0][3]);
            }
        }
    }

    #[test]
    fn test_basic_tabulation() {
        let start = Instant::now();

        let data_root = String::from("tests/data_root");
        let (ctx, rq) = SimpleRequest::from_names(
            "usa",
            &["us2015b"],
            &["MARST", "GQ"],
            Some("P".to_string()),
            None,
            Some(data_root),
        )
        .expect(
            "Setting up this request and context is for a subsequent test and should always work.",
        );

        println!(
            "tabulation test startup took {} ms",
            start.elapsed().as_millis()
        );

        let tabtime = Instant::now();

        let result = tabulate(&ctx, rq);
        println!("Test tabulation took {} ms", tabtime.elapsed().as_millis());
        if let Err(ref e) = result {
            println!("{}", e);
        }

        assert!(result.is_ok(), "Should have tabulated.");
        if let Ok(tab) = result {
            let tables = tab.into_inner();
            assert_eq!(1, tables.len());
            for t in tables {
                println!(
                    "{}",
                    t.format_as_text()
                        .expect("should be able to format as text")
                );
                assert_eq!(18, t.rows.len());
                assert_eq!(4, t.rows[0].len());
            }
        }
    }
}
