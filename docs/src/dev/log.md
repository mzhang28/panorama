# Log

Notes taken during the development process, by date.

## 2025-02-24

- I love react native!!!!

## 2025-02-21

- lol crazy dividers <https://emojicombos.com/text-divider>

## 2025-02-20

- [x] file upload via local filesystem via object_store crate
- [x] Have individual views (this was done as a tab like thing)
- [x] download bun
- later when doing real time stuff, use this: <https://loro.dev/blog/loro-richtext>
- something wonky going on with closing tabs, TODO: check it again

## 2025-02-19

- [x] support start/end query
- Wakapi actually lets u export the data as CSV, use this to build and test the dataviz software
  - <https://github.com/muety/wakapi/blob/master/README.md#-data-export>

## older

need a good data structure that has these functions:

- insert(self) -> int
- assign(self, rowid: int, key: string, value: arbitrary) -> ()
- query(self, keys: set<string>) -> row[]

s.t. forall self, id, key, value:

self.assign(id, key, value)
assert self.query({ key }) contains id

a lattice?
