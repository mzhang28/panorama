// File Manager Plugin — React UI component with file browser and upload.
// Loaded by the Panorama host via Module Federation at runtime.

import { useEffect, useRef, useState } from "react";

// ── Minimal API helper (self-contained; no dependency on host client) ────────

async function callPluginEndpoint(
  pluginId: string,
  endpoint: string,
  method = "GET",
  body?: unknown,
): Promise<Response> {
  const opts: RequestInit = {
    method,
    headers: body ? { "Content-Type": "application/json" } : {},
    body: body ? JSON.stringify(body) : undefined,
  };
  return fetch(`/plugin/${pluginId}/${endpoint}`, opts);
}

// ── Types ───────────────────────────────────────────────────────────────────

interface Node {
  id: string;
  fields: Record<string, { type: string; value: unknown }>;
  updated_at: string;
}

interface FilesAppProps {
  pluginId: string;
}

// ── Component ───────────────────────────────────────────────────────────────

export default function FilesApp({ pluginId }: FilesAppProps) {
  const PLUGIN_ID = pluginId || "io.mzhang.panorama.files";
  const [files, setFiles] = useState<Node[]>([]);
  const [loading, setLoading] = useState(true);
  const [uploading, setUploading] = useState(false);
  const [dragOver, setDragOver] = useState(false);
  const fileInputRef = useRef<HTMLInputElement>(null);

  const fetchFiles = async () => {
    setLoading(true);
    try {
      const res = await callPluginEndpoint(PLUGIN_ID, "files");
      setFiles(await res.json());
    } catch (_e) {}
    setLoading(false);
  };

  useEffect(() => {
    fetchFiles();
  }, []);

  const handleUpload = async (file: File) => {
    setUploading(true);
    try {
      const url = `/plugin/${PLUGIN_ID}/upload?filename=${encodeURIComponent(file.name)}&mime_type=${encodeURIComponent(file.type || "application/octet-stream")}`;
      await fetch(url, { method: "POST", body: file });
      fetchFiles();
    } catch (_e) {}
    setUploading(false);
  };

  const handleDelete = async (id: string) => {
    if (!confirm("Delete this file?")) return;
    await callPluginEndpoint(PLUGIN_ID, `files/${id}`, "DELETE");
    fetchFiles();
  };

  const handleDownload = (id: string) => {
    window.open(`/plugin/${PLUGIN_ID}/files/${id}`, "_blank");
  };

  const onDrop = (e: React.DragEvent) => {
    e.preventDefault();
    setDragOver(false);
    const file = e.dataTransfer.files[0];
    if (file) handleUpload(file);
  };

  const formatSize = (bytes: number) => {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  };

  return (
    <div style={{ maxWidth: 800, margin: "0 auto" }}>
      <h2 style={{ marginBottom: 16 }}>File Manager</h2>

      {/* Upload zone */}
      <div
        className="card"
        onDragOver={(e) => {
          e.preventDefault();
          setDragOver(true);
        }}
        onDragLeave={() => setDragOver(false)}
        onDrop={onDrop}
        style={{
          border: `2px dashed ${dragOver ? "var(--accent)" : "var(--border)"}`,
          textAlign: "center",
          padding: 32,
          marginBottom: 20,
          cursor: "pointer",
          background: dragOver ? "var(--bg-hover)" : undefined,
        }}
        onClick={() => fileInputRef.current?.click()}
      >
        <p style={{ fontSize: 32, marginBottom: 8 }}>
          {uploading ? "⏳" : "📁"}
        </p>
        <p>
          {uploading ? "Uploading..." : "Drop files here or click to upload"}
        </p>
        <p className="text-muted" style={{ fontSize: 13 }}>
          Supports any file type · Resumable uploads available for large files
        </p>
        <input
          ref={fileInputRef}
          type="file"
          style={{ display: "none" }}
          onChange={(e) => {
            const f = e.target.files?.[0];
            if (f) handleUpload(f);
          }}
        />
      </div>

      {/* File list */}
      {loading ? (
        <p>Loading...</p>
      ) : (
        <div>
          <p className="text-muted" style={{ marginBottom: 8 }}>
            {files.length} file{files.length !== 1 ? "s" : ""}
          </p>
          <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
            {files.map((f) => {
              const filename =
                (f.fields["system:node_title"]?.value as string) || "unnamed";
              const size = (f.fields["files:file_size"]?.value as number) || 0;
              const mime =
                (f.fields["files:mime_type"]?.value as string) || "unknown";
              return (
                <div
                  key={f.id}
                  className="card flex-row"
                  style={{ justifyContent: "space-between" }}
                >
                  <div>
                    <strong>{filename}</strong>
                    <div className="text-muted" style={{ fontSize: 12 }}>
                      {formatSize(size)} · {mime} ·{" "}
                      {new Date(f.updated_at).toLocaleDateString()}
                    </div>
                  </div>
                  <div className="flex-row" style={{ gap: 4 }}>
                    <button onClick={() => handleDownload(f.id)}>
                      ⬇ Download
                    </button>
                    <button
                      onClick={() => handleDelete(f.id)}
                      style={{ color: "var(--danger)" }}
                    >
                      Delete
                    </button>
                  </div>
                </div>
              );
            })}
            {files.length === 0 && (
              <p className="text-muted">No files uploaded yet.</p>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
