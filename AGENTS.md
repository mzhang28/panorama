This is panorama, an everything app similar to Notion, Anytype, Logseq.

# Architecture

The frontend is written in Qt.
The backend is written in Rust.

The data structure presented to the frontend is essentially a graph of nodes.
A node can have multiple fields associated to it, each from different apps.
This way, a single node can have multiple meanings.

The real database is SQLite, with a single nodes table and a bunch of dynamically managed tables that is tracked via `_panorama_schema*` tables.

Fields are a distinct concept from edges, which relate nodes.

# Operating instructions

- Your modifications are not complete until both Rust and C++ sides compile. This is done by running `ninja -C build` from the root directory of this repository.
