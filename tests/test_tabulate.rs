//! Tabulation integration tests
use cimdea::request::AbacusRequest;
use cimdea::tabulate::tabulate;

/// This test tabulates a single P variable MARST, which does not have category
/// bins. There are no subpopulations applied.
#[test]
fn test_no_category_bins_no_subpopulations() {
    let input_json = include_str!("requests/no_category_bins_no_subpops.json");
    let (ctx, request) =
        AbacusRequest::try_from_json(input_json).expect("should be able to parse input JSON");
    let tab = tabulate(&ctx, request).expect("tabulation should run without errors");
    let tables = tab.into_inner();

    assert_eq!(tables.len(), 1);
    let table = tables[0].clone();

    assert_eq!(table.heading.len(), 3);
    assert_eq!(table.heading[0].name(), "ct");
    assert_eq!(table.heading[1].name(), "weighted_ct");
    assert_eq!(table.heading[2].name(), "MARST");

    assert_eq!(table.rows.len(), 6);
    for row in &table.rows {
        assert_eq!(row.len(), 3);
    }

    // Check ct
    assert_eq!(table.rows[0][0], "10050");
    assert_eq!(table.rows[1][0], "499");
    assert_eq!(table.rows[2][0], "707");
    assert_eq!(table.rows[3][0], "3670");
    assert_eq!(table.rows[4][0], "2267");
    assert_eq!(table.rows[5][0], "13574");

    // Check weighted_ct
    assert_eq!(table.rows[0][1], "998208");
    assert_eq!(table.rows[1][1], "54103");
    assert_eq!(table.rows[2][1], "82407");
    assert_eq!(table.rows[3][1], "404131");
    assert_eq!(table.rows[4][1], "204365");
    assert_eq!(table.rows[5][1], "1730968");

    // Check MARST
    assert_eq!(table.rows[0][2], "1");
    assert_eq!(table.rows[1][2], "2");
    assert_eq!(table.rows[2][2], "3");
    assert_eq!(table.rows[3][2], "4");
    assert_eq!(table.rows[4][2], "5");
    assert_eq!(table.rows[5][2], "6");
}
