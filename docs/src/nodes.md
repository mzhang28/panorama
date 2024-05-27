# Nodes

Everything is organized into nodes.

Each app (journal, mail, etc.) creates relations from node IDs to their information.

For example, in a journal, there would be 2 database entries:

- `node { id: "12345" => type: "panorama/journal/page", created_at: (...), ... }`
- `journal { node_id: "12345" => content: "blah blah blah" }`

When retrieving its contents, a join relation is conducted and all the fields are returned.

## Field mapping

In the database, there is a relation mapping field names that the frontend knows about, such as `panorama/journal/page/content` to the actual relation (`journal`) + field name (`content`). These are currently all hard-coded into the migrations, but when custom apps are added they will be able to be registered.

## Types

The node type tells the frontend how to render it.

**TODO:** when custom apps hit, what's the best way to package frontend React code?

## Synthetic nodes

These nodes basically only exist on the frontend. For example, `panorama/mail` is a special ID that renders the mail page.

**TODO:** consider replacing these with short-circuiting the query instead of having special IDs?