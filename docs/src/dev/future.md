# Future Work

My development philosophy for this proejct revolves around aggressively rewriting until I get it right.
To fight scope creep, I push off certain features to later rewrites.
Here are some of those features:

- Sync between devices with CRDTs
- Pick a better DB maybe?
  - Go back to cozo db lmao
    - Main thing that sucks about this is not having a query builder. It would be nice to have some kind of uniform querying API that I control
- Flexible layout management + saving that layout
- Custom RPC framework for allowing progress notifications to be sent back to the client
