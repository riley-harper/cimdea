//! Code for modeling and parsing the incoming JSON schema for extract requests

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::mderror::MdError;

#[derive(Deserialize, Serialize)]
pub struct AbacusRequest {
    product: String,
    data_root: String,
    uoa: String,
    output_format: String,
    subpopulation: Vec<RequestVariable>,
    category_bins: BTreeMap<String, Vec<CategoryBinRaw>>,
    request_samples: Vec<RequestSample>,
    request_variables: Vec<RequestVariable>,
}

#[derive(Clone, Debug)]
pub enum CategoryBin {
    LessThan { value: i64, label: String },
    Range { low: i64, high: i64, label: String },
    MoreThan { value: i64, label: String },
}

impl TryFrom<CategoryBinRaw> for CategoryBin {
    type Error = MdError;

    fn try_from(value: CategoryBinRaw) -> Result<Self, Self::Error> {
        let label = &value.value_label;
        match (value.low, value.high) {
            (Some(low), Some(high)) if high < low => Err(MdError::Msg(format!(
                "category_bins: a low of {} and high of {} do not satisfy low <= high",
                low, high
            ))),
            (Some(low), Some(high)) => Ok(Self::Range {
                low,
                high,
                label: label.to_owned(),
            }),
            (None, Some(high)) => Ok(Self::LessThan {
                value: high,
                label: label.to_owned(),
            }),
            (Some(low), None) => Ok(Self::MoreThan {
                value: low,
                label: label.to_owned(),
            }),
            (None, None) => Err(MdError::Msg(
                "category_bins: must have low, high, or both set to some value".to_string(),
            )),
        }
    }
}

impl CategoryBin {
    pub fn new(low: Option<i64>, high: Option<i64>, label: &str) -> Result<Self, MdError> {
        match (low, high) {
            (Some(low), Some(high)) if high < low => Err(MdError::Msg(format!(
                "category_bins: a low of {} and high of {} do not satisfy low <= high",
                low, high
            ))),
            (Some(low), Some(high)) => Ok(Self::Range {
                low,
                high,
                label: label.to_owned(),
            }),
            (None, Some(high)) => Ok(Self::LessThan {
                value: high,
                label: label.to_owned(),
            }),
            (Some(low), None) => Ok(Self::MoreThan {
                value: low,
                label: label.to_owned(),
            }),
            (None, None) => Err(MdError::Msg(
                "category_bins: must have low, high, or both set to some value".to_string(),
            )),
        }
    }

    pub fn within(&self, test_value: i64) -> bool {
        match self {
            Self::LessThan { value, .. } => test_value < *value,
            Self::Range { low, high, .. } => test_value >= *low && test_value <= *high,
            Self::MoreThan { value, .. } => test_value > *value,
        }
    }
}
#[derive(Deserialize, Serialize)]
pub struct CategoryBinRaw {
    code: usize,
    value_label: String,
    low: Option<i64>,
    high: Option<i64>,
}

#[derive(Deserialize, Serialize)]
pub struct RequestVariable {
    variable_mnemonic: String,
    mnemonic: String,
    general_detailed_selection: String,
    attached_variable_pointer: (),
    case_selection: bool,
    request_case_selections: Vec<RequestCaseSelection>,
    extract_start: usize,
    extract_width: usize,
}

#[derive(Deserialize, Serialize)]
pub struct RequestSample {
    name: String,
    custom_sampling_ratio: Option<String>,
    first_household_sampled: Option<usize>,
}

#[derive(Deserialize, Serialize)]
pub struct RequestCaseSelection {
    low_code: String,
    high_code: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Deserialize a real example AbacusRequest with the request variables
    /// INCWAGE and MARST.
    #[test]
    fn test_deserialize_incwage_marst_example() {
        let json_str = include_str!("../test/requests/incwage_marst_example.json");
        let request: AbacusRequest =
            serde_json::from_str(json_str).expect("should deserialize into an AbacusRequest");

        assert_eq!(request.product, "usa");
        assert_eq!(request.category_bins["INCWAGE"].len(), 17);
    }

    #[test]
    fn test_category_bin_new_less_than() {
        let bin = CategoryBin::new(None, Some(3), "less than 3")
            .expect("expected Ok(CategoryBin::LessThan)");
        assert!(matches!(bin, CategoryBin::LessThan { .. }))
    }

    #[test]
    fn test_category_bin_try_from_less_than() {
        let raw_bin = CategoryBinRaw {
            code: 0,
            value_label: "less than 3".to_string(),
            low: None,
            high: Some(3),
        };
        let bin = CategoryBin::try_from(raw_bin)
            .expect("should successfully convert from CategoryBinRaw");
        assert!(matches!(bin, CategoryBin::LessThan { .. }))
    }

    #[test]
    fn test_category_bin_new_more_than() {
        let bin = CategoryBin::new(Some(3), None, "more than 3")
            .expect("expected Ok(CategoryBin::MoreThan)");
        assert!(matches!(bin, CategoryBin::MoreThan { .. }));
    }

    #[test]
    fn test_category_bin_try_from_more_than() {
        let raw_bin = CategoryBinRaw {
            code: 0,
            value_label: "more than 3".to_string(),
            low: Some(3),
            high: None,
        };
        let bin = CategoryBin::try_from(raw_bin)
            .expect("should successfully convert from CategoryBinRaw");
        assert!(matches!(bin, CategoryBin::MoreThan { .. }));
    }

    #[test]
    fn test_category_bin_new_range() {
        let bin = CategoryBin::new(Some(3), Some(5), "between 3 and 5")
            .expect("expected Ok(CategoryBin::Range)");
        assert!(matches!(bin, CategoryBin::Range { .. }));
    }

    #[test]
    fn test_category_bin_try_from_range() {
        let raw_bin = CategoryBinRaw {
            code: 0,
            value_label: "between 3 and 5".to_string(),
            low: Some(3),
            high: Some(5),
        };
        let bin = CategoryBin::try_from(raw_bin)
            .expect("should successfully convert from CategoryBinRaw");
        assert!(matches!(bin, CategoryBin::Range { .. }));
    }

    #[test]
    fn test_category_bin_new_no_boundaries_error() {
        let result = CategoryBin::new(None, None, "no boundaries!");
        assert!(
            result.is_err(),
            "it should be an error if neither low nor high is provided"
        );
    }

    #[test]
    fn test_category_bin_try_from_no_boundaries_error() {
        let raw_bin = CategoryBinRaw {
            code: 0,
            value_label: "no boundaries!".to_string(),
            low: None,
            high: None,
        };
        let result = CategoryBin::try_from(raw_bin);
        assert!(
            result.is_err(),
            "it should be an error if neither low nor high is provided"
        );
    }

    #[test]
    fn test_category_bin_new_high_less_than_low_error() {
        let result = CategoryBin::new(Some(10), Some(2), "that's not possible");
        assert!(result.is_err(), "it should be an error if high < low");
    }

    #[test]
    fn test_category_bin_try_from_high_less_than_low_error() {
        let raw_bin = CategoryBinRaw {
            code: 0,
            value_label: "that's not possible".to_string(),
            low: Some(10),
            high: Some(2),
        };
        let result = CategoryBin::try_from(raw_bin);
        assert!(result.is_err(), "it should be an error if high < low");
    }
}
