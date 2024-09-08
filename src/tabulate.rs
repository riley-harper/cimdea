use crate::conventions::Context;
use crate::ipums_data_model::*;
use crate::ipums_metadata_model::*;
use crate::request::InputType;
use crate::request::SimpleRequest;

pub enum TableFormat {
    Csv,
    Html,
    Json,
    TextTable,
}

// If we want we can use the IpumsVariable categories to replace the numbers in the results (rows)
// with category labels and use the data type and width information to better format the table.
pub struct Table {
    pub heading: Vec<IpumsVariable>, // variable name columns
    pub count: String,
    pub weighted_count: Option<String>,
    pub weight_variable: Option<IpumsVariable>,
    pub rows: Vec<Vec<String>>,
}

impl Table {
    pub fn output(&self, format: TableFormat) -> String {
        match format {
            TableFormat::Html | TableFormat::Csv | TableFormat::Json => {
                panic!("Output format not implemented yet.")
            }
            TableFormat::TextTable => self.formatAsText(),
        }
    }

    pub fn formatAsText(&self) -> String {
        let mut out = String::new();
        let widths = self.column_widths();

        for (column, v) in self.heading.iter().enumerate() {
            let name = self.heading[column].name.clone();
            let column_header = format!("| {:>1$} |", &name, widths[column]);
            out.push_str(&column_header);
        }
        out.push_str("\n");
        out.push_str(&format!("{:-0$}", self.text_table_width()));

        for r in &self.rows {
            for (column, item) in r.iter().enumerate() {
                let w = widths[column];
                let formatted_item = format!("| {:>1$} ", &item, w);
                out.push_str(&formatted_item);
            }
            out.push_str("|\n");
        }
        return out;
    }

    pub fn text_table_width(&self) -> usize {
        1 + 3 * self.heading.len() + self.column_widths().iter().sum::<usize>()
    }

    fn column_widths(&self) -> Vec<usize> {
        let mut widths = Vec::new();
        for (column, var) in self.heading.iter().enumerate() {
            let name_width = var.name.len();
            if let Some((_, width)) = var.formatting {
                if name_width < width {
                    widths.push(width);
                } else {
                    widths.push(name_width);
                }
            } else {
                if let Some(w) = self.width_from_data(column) {
                    if name_width < w {
                        widths.push(w);
                    } else {
                        widths.push(name_width);
                    }
                } else {
                    panic!("Can't determine column width of data.");
                }
            }
        }
        widths
    }

    fn width_from_data(&self, column: usize) -> Option<usize> {
        self.rows.iter().map(|r| r[column].len()).max()
    }

    pub fn empty() -> Self {
        Self {
            rows: Vec::new(),
            heading: Vec::new(),
            count: "count".to_string(),
            weighted_count: None,
            weight_variable: None,
        }
    }
}

pub fn tabulate(ctx: &Context, rq: &SimpleRequest) -> Result<Vec<Table>, String> {
    //    let dataset_name = rq.datasets[0].name.clone();
    let tables = rq
        .datasets
        .iter()
        .map(|dataset| {
            let dataset_name = dataset.name.to_owned();
            // Construct the conventional path given the InputType, one per record type in case of non-hierarchical formats.
            let data_paths = ctx.paths_from_dataset_name(&dataset_name, &InputType::Parquet);

            let tb = Table::empty();
            tb
        })
        .collect::<Vec<Table>>();
    Ok(tables)
}
