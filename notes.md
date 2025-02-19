# TODOs

- Calendar
- Main screen
  - Have individual views
  - Make those views saveable as nodes in the db
- Storage
  - Figure out how to hook up to garage
  - For other users: Figure out how to hook up to actual S3

# Next Rewrite Tasks

- Replace the db?

# Development Log

## 2025-02-19

- [x] support start/end query

## older

need a good data structure that has these functions:

- insert(self) -> int
- assign(self, rowid: int, key: string, value: arbitrary) -> ()
- query(self, keys: set<string>) -> row[]

s.t. forall self, id, key, value:

  self.assign(id, key, value)
  assert self.query({ key }) contains id

a lattice?
