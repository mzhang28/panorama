create table nodes (
    id text primary key,
    type text not null,
    created_at datetime not null,
    updated_at datetime not null,
    extra json not null default '{}'
);

create table edges (
    from_id text not null,
    to_id text not null,
    created_at datetime not null,
    updated_at datetime not null,
    extra json not null default '{}'
);

create table _panorama_schema_columns (
    key text primary key,
    sqlite_table_name text not null,
    sqlite_column_name text not null,
    index_name text
);
