# Spec: Schema-Directed Binary Serialization & `FromNode` Derive Macro

This specification proposes a type-safe, zero-overhead binary serialization layout and macro-based mapping system for Panorama. It allows WASM guest plugins to decode query results directly into native Rust structs without the performance and allocation overhead of JSON parsing.

---

## 1. Core Traits

We define three core traits to manage the conversion from binary datatypes, nodes, and database rows to Rust structures:

### 1.1 `FromFieldValue`
Extracts a concrete Rust type from a namespaced field value.
```rust
pub trait FromFieldValue: Sized {
    fn from_field_value(value: &FieldValue) -> Result<Self, String>;
}
```

### 1.2 `FromNode`
Maps a full `Node` structure to a type-safe Rust struct using its schema mapping.
```rust
pub trait FromNode: Sized {
    fn from_node(node: &crate::types::Node) -> Result<Self, String>;
}
```

### 1.3 `FromRow`
Maps a single database query row (`serde_json::Value` or binary row buffer) to a struct.
```rust
pub trait FromRow: Sized {
    fn from_row(row: &serde_json::Value) -> Result<Self, String>;
}
```

---

## 2. Schema-Directed Binary Serialization (SDBS)

To pass data across the host-guest WASM boundary efficiently, we bypass JSON entirely. Because both the host and the WASM guest register and know the schema definitions (such as `trips/Event`) at runtime, they can dynamically derive the serialization layout.

### 2.1 Deterministic Field Ordering
To avoid transmitting field key strings (e.g. `"trips:latitude"`), the wire layout order is derived by **sorting the schema fields alphabetically by namespace and name**. 
* **Key Generation**: `key = format!("{}:{}", field.namespace, field.name)`
* Sorting is deterministic, ensuring both host and guest agree on the sequence of values without exchanging index mapping tables.

### 2.2 Wire Layout Specification
A serialized node is a contiguous stream of raw values in sorted schema field order:
* **`String`**: Varint-encoded length followed by raw UTF-8 bytes.
* **`Integer`**: 8-byte signed integer (big-endian/little-endian matching target platform, or varint-encoded).
* **`Float`**: 8-byte IEEE 754 float.
* **`Boolean`**: 1 byte (`0` or `1`).
* **`NodeRef`**: 16-byte raw UUID array.
* **`DateTime`**: Varint-encoded length followed by ISO-8601 string bytes.
* **`Optional Fields`**: Preceded by a `1` (Some) or `0` (None) presence byte.
* **`Arrays`**: Varint-encoded length prefix followed by sequential elements.

No field keys, namespaces, or type tags are transmitted.

---

## 3. The `#[derive(FromNode)]` Custom Derive

The `FromNode` derive macro generates compile-time mapping code for struct fields.

### 3.1 Attributes
* `#[panorama(schema = "name")]`: Links the struct to a registered schema.
* `#[panorama(namespace = "ns")]`: Specifies a default namespace for the struct's fields.
* `#[panorama(rename = "ns:name")]`: Explicitly maps a field to a custom namespaced field key.

### 3.2 Developer DX Example
```rust
#[derive(FromNode)]
#[panorama(schema = "trips/Event")]
pub struct Event {
    // Overrides default naming
    #[panorama(rename = "system:node_title")]
    pub title: String,
    
    // Auto-maps to "trips:latitude"
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
}
```

### 3.3 Generated Macro Code (WASM Guest Side)
At compile time, the derive macro inspects the struct and sets up sequential reads based on the deterministic alphabetical sort order of the schema:
1. `system:node_title` (String)
2. `trips:latitude` (Float, Optional)
3. `trips:longitude` (Float, Optional)

```rust
impl FromNode for Event {
    fn from_node(node: &panorama_core::types::Node) -> Result<Self, String> {
        let title = node.get_field("system:node_title")
            .ok_or_else(|| "Missing system:node_title".to_string())
            .and_then(FromFieldValue::from_field_value)?;

        let latitude = match node.get_field("trips:latitude") {
            Some(val) => FromFieldValue::from_field_value(val)?,
            None => None,
        };

        let longitude = match node.get_field("trips:longitude") {
            Some(val) => FromFieldValue::from_field_value(val)?,
            None => None,
        };

        Ok(Self {
            title,
            latitude,
            longitude,
        })
    }
}
```

---

## 4. Query Extension helper: `query_as`

We implement a blanket extension trait `PluginContextExt` for all `PluginContext` trait objects to support direct query-to-struct execution:

```rust
#[async_trait]
pub trait PluginContextExt: PluginContext {
    async fn query_as<T: FromRow>(&self, query_string: &str) -> Result<Vec<T>, PluginError> {
        let rows = self.query(query_string).await?;
        let mut results = Vec::with_capacity(rows.len());
        for row in rows {
            let mapped = T::from_row(&row)
                .map_err(|e| PluginError::internal(format!("Decode error: {}", e)))?;
            results.push(mapped);
        }
        Ok(results)
    }
}

impl<C: PluginContext + ?Sized> PluginContextExt for C {}
```

---

## 5. Implementation Roadmap
1. Create a helper proc-macro crate `crates/panorama-macros` in the workspace.
2. Define traits `FromFieldValue`, `FromNode`, and `FromRow` inside `crates/panorama-core/src/query/decode.rs`.
3. Update `wasm_adapter.rs` and `wasm_runtime.rs` to exchange query results using `postcard` binary frames derived from schema lists rather than raw JSON strings.
4. Export the custom derives directly from `panorama-core` for plug-and-play developer usage.
