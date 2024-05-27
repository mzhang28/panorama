# Custom Apps

<div class="warning">

**WARNING:** This is documentation for a feature that is in development.

Almost none of this is implemented and most of it will probably change in the future.

</div>

Custom apps allow third parties to develop functionality for panorama.
After this rolls out, most of the built-in panorama apps will also be converted into custom apps, and this feature will just be renamed "apps".

## API

To develop a custom app, you will need to provide:

-
  App metadata. This contains:

  - App display name.
  - Version + License.
  - Description + Keywords.
  - Compatible panorama versions (TODO).
  - Authors + Maintainers.
  - Repository + Issues.
  - Extra data fields for whatever
  
  This also includes relationships with other apps. For example:

  - Field read dependencies. If your app needs to read for example `panorama/std/time`, then it needs to list it.
  - Field write dependencies. This breaks down to:
    - any: the app is allowed to write to the specified field on any node
    - owned: the app is allowed to write to the specified field on nodes it owns (**TODO** flesh out app ownership of nodes)
    - none: the app isn't allowed to write to the specified field

-
  A list of relations your app will use.

  For example, the journal app will use `journal` for keeping track of regular pages, but may use another relation `journal_day` for keeping track of mapping days to journals. (**TODO:** not a good example, these could be combined)

  The indexes for the relations should also be listed.
-
  A list of services your app will run in the background.

## App ownership of nodes

Apps automatically own nodes they create.

**TODO:** is multiple ownership allowed?

## Design notes

-
  Maybe it's best to generate the actual db relation names and have their symbolic names be mapped? This will require an extra layer of indirection but it should still make querying be doable in 2 queries.

  For example, the journal app specifies that it wants a `journal` relation. The db generates something like `journal_a41e`, registers that as a mapping for the "journal" app, and all queries will actually involve that name.

  This avoids name conflicts for separate third parties that use the same name for a relation.


## Built-in apps

### Journal

### Mail

### Calendar

### Contacts