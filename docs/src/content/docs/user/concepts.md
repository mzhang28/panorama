---
title: Core Concepts
description: Detailed explanation of Nodes, Fields, Schemas, Spaces, and Object Storage in Panorama.
---

Panorama is a self-hosted data layer designed around five foundational primitives. Understanding these core concepts is key to developing applications and managing data within the platform.

---

## 1. Nodes

A **Node** is the primary unit of storage in Panorama. Unlike relational databases that use rigid tables, or document databases that store hierarchical objects, Panorama stores everything as a flat node. 

Every node consists of:
*   **ID**: A unique UUID (`id`) generated on creation.
*   **Space ID**: A UUID (`space_id`) linking the node to a specific multi-user authorization boundary.
*   **Fields**: Arbitrary namespaced key-value pairs (e.g. `system:node_title`, `journal:content`).
*   **Schema Conformance**: Meta-records linking the node to schemas it conforms to.
*   **System Timestamps**: Automated UTC timestamps (`created_at` and `updated_at`).

---

## 2. Fields

Fields are namespaced key-value pairs representing node attributes. 

### Namespaces
Namespaces prevent name collisions between apps and system fields. They follow the format `<namespace>:<field_name>`:
*   `system:` — Reserved for core platform fields (e.g., `system:node_title`, `system:node_time`, `system:created_at`).
*   `user:` — Reserved for ad-hoc fields created directly by users.
*   `<app-id>:` — Reserved for installed applications (e.g., `journal:content`, `coding:project`).

### Field Types
Fields support various types, including:
*   `String`, `Integer`, `Float`, `Boolean`
*   `Timestamp` (ISO 8601 formatted string)
*   `Ref` (a link to another node's UUID)
*   `ObjectRef` (a link to binary data in Object Storage)
*   **CRDT Types**: Specialized fields that merge concurrently (e.g., `OrSet`, `Counter`, `RgaText`).

---

## 3. Schemas

A **Schema** declares structural expectations for nodes. It specifies expected fields, types, and validation rules. Because schemas are themselves represented as nodes, the system is fully self-describing.

Schemas have two primary operating modes:

### Preferred Schemas (Gradual Compliance)
*   Nodes can deviate from the schema (e.g., fields missing or types mismatched).
*   Writing invalid data is permitted, but the platform raises warnings that are visible in the UI.
*   Ideal for evolutionary, loose data models where app-level leniency is expected.

### Required Schemas (Strict Compliance)
*   Nodes **must** always conform to the schema constraints.
*   Any database transaction attempting to write non-conforming data is immediately rejected and rolled back.
*   Ensures strong data consistency for critical application states.

Schemas are versioned (using major/minor increments) to coordinate migrations safely as application requirements change.

---

## 4. Spaces

A **Space** represents a data partition and permission boundary. 

*   All nodes reside inside exactly one space (referenced by `space_id`).
*   Instead of node-level access lists, permissions are managed globally at the space level (the Anytype permissions model).
*   Spaces can be shared with other users or made public, making all nodes inside them subject to the same access permissions.
*   Queries cannot scan across multiple spaces implicitly; all queries must scope their search to a single space using an explicit space filter.

---

## 5. Object Storage

Large files (images, audio, database backups, archives) are inefficient to store inline as node fields. Panorama provides a built-in **Object Storage** system:

*   **Blob Store**: S3-like binary storage organized by bucket and key.
*   **ObjectRef**: A special field type stored on nodes. A node holds a reference metadata pointer to a blob (e.g., `my_photo.jpg`), rather than raw bytes.
*   **Resumable Uploads**: Supports chunked, resumable uploads for large files (critical for network-constrained personal servers).
