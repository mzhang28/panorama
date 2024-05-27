# Sync

<div class="warning">

**WARNING:** This is documentation for a feature that is in development.

Almost none of this is implemented and most of it will probably change in the future.

</div>

This **only** deals with syncing nodes and files between devices owned by the same person. Permissions are not considered here.

## Design notes

-
  Devices need to have some kind of knowledge of each other's existence. This may not necessarily be exposed to apps, but the thing that's responsible for syncing needs to know which nodes have which files.
-
  Slow internet connections and largely offline usage patterns need to be considered.
-
  **TODO:** does this need to be deeply integrated within the panorama daemon itself or is there a way to expose enough APIs for this to just be an app?
