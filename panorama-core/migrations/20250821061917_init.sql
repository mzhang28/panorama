create table if not exists nodes (
    id text primary key,
    created_at datetime not null,
    updated_at datetime not null,
    extra json not null default '{}'
);

create table if not exists edges (
    from_id text not null,
    to_id text not null,
    created_at datetime not null,
    updated_at datetime not null,
    extra json not null default '{}'
);

create table if not exists _panorama_schema (
    key text primary key,
    sqlite_table_name text not null,
    sqlite_column_name text not null,
    index_name text
);
