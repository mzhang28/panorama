# Node info

Nodes are currently just a row in an SQLite database, with certain columns enabled.

CHALLENGES:

- How to indicate what "type" a node is?
  - is this even well defined? given that a node may have multiple types
  - maybe I need to make concrete the concept of an _interface_.
    I'm suspicious that this concept will have to exist _outside_ the SQL schema and then validated.
  - It's possible that prisma may help a lot in this regard
    <https://github.com/prisma/prisma/discussions/4425#discussioncomment-143293>
- Could use MIME type to indicate some sort of "primary" type, and then have a list of subtypes that it correctly implements.
  - This would make choosing how to display it a lot easier
  - I think in order to determine what other interfaces it implements, I would have to have those interface maps live outside the SQL
    - Since custom apps don't exist yet, this could just be some JSON file

## Querying

{a = asdf} AND ({b > 5} OR asdf uiop)

Syntax:

- Field constraints wrapped in {...}
- Obviously parentheses for order of operations
- Otherwise text keywords
- I think I will make AND/OR have the same precedence and not associate, to avoid ambiguous situations

Result:

- This needs to be compiled to an SQL statement + bind, and sent over to the database
- TODO: Are there any features here that CANNOT be compiled to SQL?
