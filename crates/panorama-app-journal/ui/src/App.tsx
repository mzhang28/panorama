// Journal Plugin — React UI component
// Displays journal entries stacked newest-first with a create form.
// Loaded by the Panorama host via Module Federation at runtime.

import { useState } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";

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

interface JournalAppProps {
  pluginId: string;
}

// ── Component ───────────────────────────────────────────────────────────────

export default function JournalApp({ pluginId }: JournalAppProps) {
  const PLUGIN_ID = pluginId || "io.mzhang.panorama.journal";
  const queryClient = useQueryClient();

  const [title, setTitle] = useState("");
  const [content, setContent] = useState("");
  const [mood, setMood] = useState("");
  const [error, setError] = useState("");
  const [selectedEntry, setSelectedEntry] = useState<string | null>(null);

  const {
    data: entries = [],
    isLoading,
    isError,
  } = useQuery<Node[]>({
    queryKey: [PLUGIN_ID, "entries"],
    queryFn: async () => {
      const res = await callPluginEndpoint(PLUGIN_ID, "entries");
      if (!res.ok) throw new Error(await res.text());
      return res.json();
    },
  });

  const createMutation = useMutation({
    mutationFn: async () => {
      const res = await callPluginEndpoint(PLUGIN_ID, "entries", "POST", {
        title,
        content,
        mood: mood || undefined,
      });
      if (!res.ok) {
        const msg = await res.text();
        throw new Error(msg);
      }
      return res.json();
    },
    onSuccess: () => {
      setTitle("");
      setContent("");
      setMood("");
      setError("");
      queryClient.invalidateQueries({ queryKey: [PLUGIN_ID, "entries"] });
    },
    onError: (e: Error) => {
      setError(e.message);
    },
  });

  const moods = [
    "happy",
    "thoughtful",
    "tired",
    "excited",
    "anxious",
    "grateful",
    "neutral",
  ];

  return (
    <div style={{ maxWidth: 720, margin: "0 auto" }}>
      <h2 style={{ marginBottom: 16 }}>Journal</h2>

      {/* Create form */}
      <div className="card" style={{ marginBottom: 20 }}>
        <input
          placeholder="Entry title"
          value={title}
          onChange={(e) => setTitle(e.target.value)}
          style={{ width: "100%", marginBottom: 8 }}
        />
        <textarea
          placeholder="Write your entry (markdown supported)..."
          value={content}
          onChange={(e) => setContent(e.target.value)}
          rows={5}
          style={{ width: "100%", marginBottom: 8 }}
        />
        <div className="flex-row" style={{ marginBottom: 8 }}>
          <select value={mood} onChange={(e) => setMood(e.target.value)}>
            <option value="">No mood</option>
            {moods.map((m) => (
              <option key={m} value={m}>
                {m}
              </option>
            ))}
          </select>
          <button
            className="primary"
            onClick={() => createMutation.mutate()}
            disabled={createMutation.isPending || !title || !content}
          >
            {createMutation.isPending ? "Saving..." : "Save Entry"}
          </button>
        </div>
        {error && <p style={{ color: "var(--danger)" }}>{error}</p>}
      </div>

      {/* Entry list */}
      {isLoading ? (
        <p>Loading...</p>
      ) : isError ? (
        <p style={{ color: "var(--danger)" }}>Failed to load entries.</p>
      ) : (
        <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
          {entries.map((entry) => (
            <div
              key={entry.id}
              className="card"
              style={{ cursor: "pointer" }}
              onClick={() =>
                setSelectedEntry(selectedEntry === entry.id ? null : entry.id)
              }
            >
              <div
                className="flex-row"
                style={{ justifyContent: "space-between" }}
              >
                <strong>
                  {(entry.fields["system:node_title"]?.value as string) ||
                    "Untitled"}
                </strong>
                <span className="text-muted">
                  {new Date(entry.updated_at).toLocaleDateString()}
                </span>
              </div>
              {entry.fields["journal:mood"] && (
                <span className="text-muted" style={{ fontSize: 12 }}>
                  Mood: {(entry.fields["journal:mood"].value as string) ?? ""}
                </span>
              )}
              {selectedEntry === entry.id && (
                <div
                  style={{
                    marginTop: 12,
                    padding: 12,
                    background: "var(--bg)",
                    borderRadius: 6,
                  }}
                >
                  <pre
                    style={{
                      whiteSpace: "pre-wrap",
                      fontFamily: "inherit",
                    }}
                  >
                    {(entry.fields["journal:content"]?.value as string) ??
                      "(no content)"}
                  </pre>
                </div>
              )}
            </div>
          ))}
          {entries.length === 0 && (
            <p className="text-muted">
              No entries yet. Write your first journal entry!
            </p>
          )}
        </div>
      )}
    </div>
  );
}
