import { useState } from "react";
import { Link } from "@tanstack/react-router";
import { callPluginEndpoint, PluginInfo } from "../api/client";

export function PluginPanel({
  plugins,
  selectedId,
  onSelect,
}: {
  plugins: PluginInfo[];
  selectedId?: string | null;
  onSelect?: (id: string) => void;
}) {
  return (
    <div>
      <h2>Plugins</h2>
      <p className="text-muted mb-1">
        Plugins are third-party apps loaded via the Panorama plugin API
      </p>

      <div style={{ display: "grid", gap: 8, marginTop: 16 }}>
        {plugins.map((p) => (
          <Link
            key={p.id}
            to="/app/$pluginId"
            params={{ pluginId: p.id }}
            style={{
              display: "block",
              textDecoration: "none",
              color: "inherit",
              borderColor: selectedId === p.id ? "var(--accent)" : undefined,
            }}
            className="card"
            onClick={() => onSelect?.(p.id)}
          >
            <div
              className="flex-row"
              style={{ justifyContent: "space-between" }}
            >
              <div>
                <strong>{p.name}</strong>
                <span className="text-muted" style={{ marginLeft: 8 }}>
                  v{p.version}
                </span>
              </div>
              <span className="text-muted">{p.id}</span>
            </div>
            <p className="text-muted mt-1">{p.description}</p>
            <div className="flex-row mt-1">
              <span className="text-muted">
                {p.endpoints.length} endpoints · {p.ui_components.length} UI
                components
              </span>
            </div>
          </Link>
        ))}
      </div>

      {selectedId && (
        <PluginDetail plugin={plugins.find((p) => p.id === selectedId)!} />
      )}
    </div>
  );
}

function PluginDetail({ plugin }: { plugin: PluginInfo }) {
  const [testResult, setTestResult] = useState<string>("");
  const [testEndpoint, setTestEndpoint] = useState("");

  const testPluginEndpoint = async () => {
    try {
      const res = await callPluginEndpoint(
        plugin.id,
        testEndpoint || plugin.endpoints[0]?.path || "",
      );
      const data = await res.json();
      setTestResult(JSON.stringify(data, null, 2));
    } catch (e: any) {
      setTestResult(`Error: ${e.message}`);
    }
  };

  return (
    <div className="card mt-2">
      <h3>{plugin.name}</h3>
      <p className="text-muted">{plugin.description}</p>
      <p>
        ID: {plugin.id} · Version: {plugin.version}
      </p>

      <h4 style={{ marginTop: 16 }}>HTTP Endpoints</h4>
      <div className="flex-col" style={{ gap: 4 }}>
        {plugin.endpoints.map((ep) => (
          <div
            key={ep.path}
            className="flex-row"
            style={{
              padding: "4px 8px",
              background: "var(--bg)",
              borderRadius: 4,
            }}
          >
            <span
              style={{ fontWeight: 600, minWidth: 50, color: "var(--accent)" }}
            >
              {ep.method}
            </span>
            <span>
              /plugin/{plugin.id}
              {ep.path}
            </span>
            <span className="text-muted">— {ep.description}</span>
          </div>
        ))}
      </div>

      <h4 style={{ marginTop: 16 }}>UI Components</h4>
      <div className="flex-col" style={{ gap: 4 }}>
        {plugin.ui_components.map((c) => (
          <div key={c.id} className="text-muted">
            {c.name} →{" "}
            {typeof c.mount_point === "string"
              ? c.mount_point
              : c.mount_point.type || "main"}
          </div>
        ))}
      </div>

      <h4 style={{ marginTop: 16 }}>Test Endpoint</h4>
      <div className="flex-row">
        <input
          placeholder="Endpoint path (e.g., entries)"
          value={testEndpoint}
          onChange={(e) => setTestEndpoint(e.target.value)}
          style={{ flex: 1 }}
        />
        <button className="primary" onClick={testPluginEndpoint}>
          Call
        </button>
      </div>
      {testResult && (
        <pre
          style={{
            marginTop: 8,
            padding: 12,
            background: "var(--bg)",
            borderRadius: 4,
            overflow: "auto",
            maxHeight: 300,
            fontSize: 13,
          }}
        >
          {testResult}
        </pre>
      )}
    </div>
  );
}
