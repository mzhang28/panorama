use anyhow::Result;
use graphql_parser::query::{Definition, Document, OperationDefinition, Selection};
use sqlx::{QueryBuilder, Sqlite};

pub fn graphql_query_to_sql_query<'a>(
    doc: Document<'a, &'a str>,
) -> Result<QueryBuilder<'a, Sqlite>> {
    let qb = QueryBuilder::new("");

    // TODO: Check that there aren't multiple selection sets
    assert!(
        doc.definitions
            .iter()
            .filter(|def| matches!(
                def,
                Definition::Operation(OperationDefinition::SelectionSet(_))
            ))
            .count()
            == 1,
        "Multiple selection sets are not supported"
    );

    for def in doc.definitions.into_iter() {
        println!("Definition: {def:?}");

        match def {
            Definition::Operation(OperationDefinition::SelectionSet(selection_set)) => {
                for item in selection_set.items {
                    match item {
                        Selection::Field(field) => {
                            println!("Field: {:?}", field);
                        }
                        Selection::FragmentSpread(fragment_spread) => todo!(),
                        Selection::InlineFragment(inline_fragment) => todo!(),
                    }
                }
            }
            Definition::Operation(operation_definition) => todo!(),
            Definition::Fragment(fragment_definition) => todo!(),
        }
    }

    Ok(qb)
}
