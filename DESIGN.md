# Panorama

We are making a data layer for building apps on later.

Here are the basic high-level design requirements:

- The basic data structure is a node.
  - Nodes all have a unique ID (UUID is fine).
  - Nodes can have arbitrary metadata which i will refer to as "fields" attached to it.
  - The nodes are not relational, but we can build relational indexes over it.
    However, as we will see later, this will have some asterisks.
- Nodes can have schemas.
  - Schemas are basically groups of fields, along with some basic requirements of them.
  - Schemas can either be preferred or required.
    - Preferred schemas allow their nodes to fall out of schema, however, it will raise a warning about why
      - Preferred schemas are set using a designated system-level field on the node
    - Required schemas do not allow their nodes to fall out of schema.
      This comes at the cost of performance of verifying it, of course.
      (Some verification MAY be costly, although most should stay cheap)
    - Think of this spectrum kind of like gradual typing.
      You get benefits from having it be required, but not all have to be
  - Schemas may be used to inform things like how UIs display certain nodes.
    - UIs can also be made for preferred schemas, they'll just have to be a bit lenient when the node isn't compliant, maybe even with a warning of how they are noncompliant.
  - Schemas are versioned, and come with some built-in notion of compatibility (semver? or maybe just a major-minor)
    - Owners of schemas are encouraged to write migration scripts for updates
    - Migration scripts are encouraged to keep old versions around in separate nodes (remember u cna just create arbitrary fields like "old X")
  - TODO: Think about how to design around computed field updates based on dependencies and whether or not to track reverse dependencies
    - i think the design of this will HEAVILY be influenced by usage patterns, so i kinda want to just build out a v0.0 and start developing apps on it and see how it feels. i will definitely have opinions.
- Spaces will handle multi-user permissions.
  - I like Anytype's model.
    Rather than having granular individiual node-level permissions, we will just simply have space-level permissions.
  - Spaces can be shared with specific people, including being public, but all nodes under that space will share the same permissions.
    - At this time, there's no plan to have nodes that can belong to multiple spaces at once.
      - If you want to publish something to the web, say, then u will need to REPLICATE it to a public node, which can be on a pull-based schedule, at the risk of falling out of sync.
- The whole point of this is to be able to support a flexible 3rd-party app ecosystem that can access a wide spectrum of data.
  - Target apps to be supported:
    - wakatime-ish/activity-watch-ish/time tracker
    - notion-ish
    - citation db
    - email/calendar/chat
    - sensitive file storage
    - fitness/workout tracker
    - trip planner
    - all sorts of other self-hosted apps
  - Apps can define schemas
  - Apps must declare and be granted capabilities 
    - Apps must bump major version in order to chagne permission grants
      - The change in permissions can come with a reasoning if wanted
    - Even making DNS requests requires one grant per host
    - Apps can request read access to specific fields under nodes
    - There should also be allowed to "have write access to nodes it created" or something
    - Maybe a concept of app-managed node? That the user can't modify directly, for like scratch purposes
      - If a user wants to access its information, maybe they must read into it
    - Obviously also reading/writing files, executing applications, should be permissions
  - Apps can define HTTP endpoints
    - I think my original intention was for them to be on the same host.
      However, it's probably better for each app to have its own host.
    - This allows the apps to be compliant with some existing clients without having to redo the entire client
- Fields
  - By default, a user can just write in a field name, and it'll be untyped unless they also choose a type
  - However, specific fields can be chosen to be typed (i.e hold arrays, hold numbers, etc)
  - I think fields will have to be namespaced
    - the write-ins have a special user namespace
    - some system fields have a system namespace
    - The namespace should be a locally-generated ID that can be mapped to an app, but should not be tied to in the raw data storage
      - This will enable switching out different versions of apps, etc
  - I want a concept of computed fields
    - Computed fields allow referencing to other fields.
      - For example, an imageUrl can be determined via fetching a node referenced by a different field and fetching an attribute off that.
      - Can we employ JIT to make workloads faster here?
    - Just as there are preferred/required schemas, i will also allow for 3 modes of computed:
      - Eager write. The computation must occur before the write is considered complete
      - Deferred write. The computation is initiated upon write but the write can be considered complete first
      - Read. The queryer has a policy about what computations they want to invoke and will be responsible.
    - Cycle tracking
      - right now have a checker for cycles, maybe eventually a depth parameter?
    - computed fields might be allowed to be a wasm blob or at least we should try to do something JIT'able so we can have user-defined functions that run in database context
- We will have an internal object storage system API
  - Fields aren't meant to store big blobs that don't change a lot
  - Apps should really store big blobs in object storage and refer to them via a reference
  - garbage collection won't be implemented for now. we'll just have a way to sort by biggest file size and allow u to query to see what's needed
- End-to-end encryption will be nice eventually but not planned for v0.x
- Other workflows that should be able to be supported, and may need careful planning:
  - Exporting/importing
  
Other notes / questions:

- Performance is nice but not CRITICAL.
  This design is in flux, so we will tweak it based on empirical usage in panorama v0.x before going on to a stable release
- Can we make even SQL indexes as a third party app?