use anyhow::Result;
use axum::{
    Json, Router,
    extract::State,
    routing::{get, post},
};

use crate::{db::Dal, graphql::graphql_query_to_sql_query};

pub async fn server_main(dal: Dal) -> Result<()> {
    println!("Server main");
    let state = AppState { dal };
    let app = Router::new()
        .route("/", get(|| async { "Hello, World!" }))
        .route("/graphql", post(post_graphql))
        .with_state(state);

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    axum::serve(listener, app).await?;

    Ok(())
}

#[derive(Clone)]
struct AppState {
    dal: Dal,
}

#[derive(Debug, Deserialize)]
struct PostGraphqlReq {
    query: String,
}

async fn post_graphql(state: State<AppState>, request: Json<PostGraphqlReq>) {
    println!("request: {:?}", request);

    let parsed = match graphql_parser::parse_query::<&str>(&request.query) {
        Ok(doc) => doc,
        Err(e) => todo!("fucked up: {e:?}"),
    };
    println!("Parsed: {parsed:?}");

    let qb = match graphql_query_to_sql_query(parsed) {
        Ok(qb) => qb,
        Err(e) => todo!("failed to turn into sql: {e:?}"),
    };
    println!("SQL: {:?}", qb.sql());
}
