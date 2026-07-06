export function SchemaViewer({ schemas }: { schemas: any[] }) {
  return (
    <div>
      <h2>Schemas</h2>
      <p className="text-muted mb-1">
        Schemas define groups of fields with requirements
      </p>

      <div style={{ display: "grid", gap: 8, marginTop: 16 }}>
        {schemas.map((schema: any) => (
          <div key={schema.node_id} className="card">
            <div
              className="flex-row"
              style={{ justifyContent: "space-between" }}
            >
              <strong>{schema.name}</strong>
              <span className="text-muted">
                v{schema.version?.major}.{schema.version?.minor} ·{" "}
                {schema.schema_mode}
              </span>
            </div>
            <div className="flex-col mt-1" style={{ gap: 4 }}>
              {schema.fields?.map((field: any) => (
                <div
                  key={field.name}
                  className="flex-row"
                  style={{
                    padding: "4px 8px",
                    background: "var(--bg)",
                    borderRadius: 4,
                    justifyContent: "space-between",
                  }}
                >
                  <span>
                    {field.namespace}:{field.name}
                    {field.required && (
                      <span style={{ color: "var(--danger)", marginLeft: 4 }}>
                        *
                      </span>
                    )}
                  </span>
                  <span className="text-muted">
                    {field.field_type?.type_tag || "any"}
                    {field.default && " · has default"}
                    {field.computed && ` · computed(${field.computed.mode})`}
                  </span>
                </div>
              ))}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
