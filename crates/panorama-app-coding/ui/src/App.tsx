// Coding Activity Dashboard
// Shows project and language leaderboards with stats from the coding plugin.
// Loaded by the Panorama host via Module Federation at runtime.

import { useEffect, useState } from "react";

// ── API helper ────────────────────────────────────────────────────────────────

async function callPlugin(
  pluginId: string,
  endpoint: string,
  method = "GET",
  body?: unknown,
  queryParams?: Record<string, string>,
): Promise<Response> {
  const qs = queryParams
    ? "?" + new URLSearchParams(queryParams).toString()
    : "";
  const opts: RequestInit = {
    method,
    headers: body ? { "Content-Type": "application/json" } : {},
    body: body ? JSON.stringify(body) : undefined,
  };
  return fetch(`/plugin/${pluginId}/${endpoint}${qs}`, opts);
}

// ── Types ─────────────────────────────────────────────────────────────────────

interface LeaderboardItem {
  key: string;
  total_seconds: number;
  hours: number;
}

interface TimeseriesSeries {
  key: string;
  bucket: string;
  data: { time: number; iso: string; seconds: number; hours: number }[];
}

interface CodingAppProps {
  pluginId: string;
}

// ── Colors ────────────────────────────────────────────────────────────────────

const COLORS = [
  "#6c8cff",
  "#ff8c6c",
  "#6cff8c",
  "#8c6cff",
  "#ff6c8c",
  "#8cff6c",
  "#ffcc6c",
  "#6cc8ff",
  "#c86cff",
  "#6cffcc",
  "#ff6ccc",
  "#ccff6c",
];

// ── Helpers ───────────────────────────────────────────────────────────────────

function formatHours(h: number): string {
  if (h >= 1) return `${h.toFixed(1)}h`;
  if (h >= 0.01) return `${(h * 60).toFixed(0)}m`;
  return "<1m";
}

// ── Main Component ────────────────────────────────────────────────────────────

export default function CodingApp({ pluginId }: CodingAppProps) {
  const PID = pluginId || "io.mzhang.panorama.coding";
  const [projectData, setProjectData] = useState<LeaderboardItem[]>([]);
  const [languageData, setLanguageData] = useState<LeaderboardItem[]>([]);
  const [fileData, setFileData] = useState<LeaderboardItem[]>([]);
  const [timeseriesData, setTimeseriesData] = useState<TimeseriesSeries[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [timeRange, setTimeRange] = useState("7d");
  const [heartbeatInput, setHeartbeatInput] = useState(
    '{"entity":"/src/main.rs","project":"panorama","language":"Rust"}',
  );

  const fetchAllStats = async () => {
    setLoading(true);
    setError("");
    try {
      const [projRes, langRes, fileRes, tsRes] = await Promise.all([
        callPlugin(PID, "stats", "GET", null, {
          range: timeRange,
          group_by: "project",
          aggregation: "leaderboard",
        }),
        callPlugin(PID, "stats", "GET", null, {
          range: timeRange,
          group_by: "language",
          aggregation: "leaderboard",
        }),
        callPlugin(PID, "stats", "GET", null, {
          range: timeRange,
          group_by: "entity",
          aggregation: "leaderboard",
        }),
        callPlugin(PID, "stats", "GET", null, {
          range: timeRange,
          group_by: "project",
          aggregation: "timeseries",
          bucket: "day",
        }),
      ]);

      if (projRes.ok) setProjectData(await projRes.json());
      if (langRes.ok) setLanguageData(await langRes.json());
      if (fileRes.ok) setFileData(await fileRes.json());
      if (tsRes.ok) setTimeseriesData(await tsRes.json());
    } catch (e: any) {
      setError(e.message);
    }
    setLoading(false);
  };

  useEffect(() => {
    fetchAllStats();
  }, [timeRange]);

  const sendHeartbeat = async () => {
    try {
      const body = JSON.parse(heartbeatInput);
      body.time = Math.floor(Date.now() / 1000);
      await callPlugin(PID, "heartbeat", "POST", body);
      fetchAllStats();
    } catch (e: any) {
      setError(e.message);
    }
  };

  // Leaderboard bar chart
  const renderLeaderboard = (items: LeaderboardItem[], title: string) => {
    const maxH = items[0]?.hours ?? 1;
    return (
      <div className="card" style={{ padding: 16 }}>
        <h4 style={{ marginTop: 0, marginBottom: 12 }}>{title}</h4>
        {items.length === 0 ? (
          <p className="text-muted">No data yet</p>
        ) : (
          <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
            {items.slice(0, 12).map((item, i) => {
              const pct = maxH > 0 ? (item.hours / maxH) * 100 : 0;
              return (
                <div key={item.key}>
                  <div
                    style={{
                      display: "flex",
                      justifyContent: "space-between",
                      marginBottom: 2,
                      fontSize: 13,
                    }}
                  >
                    <span
                      style={{
                        overflow: "hidden",
                        textOverflow: "ellipsis",
                        whiteSpace: "nowrap",
                        maxWidth: "70%",
                      }}
                    >
                      {item.key}
                    </span>
                    <span
                      className="text-muted"
                      style={{ fontFamily: "monospace", fontSize: 12 }}
                    >
                      {formatHours(item.hours)}
                    </span>
                  </div>
                  <div
                    style={{
                      height: 6,
                      background: "var(--bg)",
                      borderRadius: 3,
                      overflow: "hidden",
                    }}
                  >
                    <div
                      style={{
                        height: "100%",
                        width: `${Math.max(pct, 1)}%`,
                        background: COLORS[i % COLORS.length],
                        borderRadius: 3,
                        transition: "width 0.5s ease",
                      }}
                    />
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </div>
    );
  };

  // Time series SVG chart
  const renderTimeseries = () => {
    if (!timeseriesData.length) return null;
    const W = 700,
      H = 200,
      PAD = { top: 12, right: 12, bottom: 30, left: 50 };
    const plotW = W - PAD.left - PAD.right;
    const plotH = H - PAD.top - PAD.bottom;

    const allPoints = timeseriesData.flatMap((s) =>
      s.data.map((d) => ({ ...d, key: s.key })),
    );
    const allTimes = allPoints.map((d) => d.time);
    const tMin = Math.min(...allTimes);
    const tMax = Math.max(...allTimes);
    const tRange = tMax - tMin || 1;
    const allVals = allPoints.map((d) => d.hours);
    const vMax = Math.max(...allVals) * 1.1 || 1;

    const toX = (t: number) => PAD.left + ((t - tMin) / tRange) * plotW;
    const toY = (v: number) => PAD.top + plotH - (v / vMax) * plotH;

    return (
      <div className="card" style={{ padding: 16 }}>
        <h4 style={{ marginTop: 0, marginBottom: 12 }}>Coding Activity</h4>
        <svg
          viewBox={`0 0 ${W} ${H}`}
          style={{ width: "100%", height: "auto", minHeight: 180 }}
        >
          {/* Y axis grid */}
          {[0, 0.25, 0.5, 0.75, 1].map((pct) => {
            const v = vMax * pct;
            const y = toY(v);
            return (
              <g key={pct}>
                <line
                  x1={PAD.left}
                  y1={y}
                  x2={W - PAD.right}
                  y2={y}
                  stroke="var(--border)"
                  strokeWidth={0.5}
                  strokeDasharray="3 2"
                />
                <text
                  x={PAD.left - 6}
                  y={y + 4}
                  textAnchor="end"
                  fill="var(--text-muted)"
                  fontSize={10}
                >
                  {`${(vMax * pct).toFixed(1)}h`}
                </text>
              </g>
            );
          })}
          {/* Data lines */}
          {timeseriesData.map((series, si) => {
            const sorted = [...series.data].sort((a, b) => a.time - b.time);
            if (sorted.length < 2) return null;
            const path = sorted
              .map(
                (p, i) =>
                  `${i === 0 ? "M" : "L"} ${toX(p.time)} ${toY(p.hours)}`,
              )
              .join(" ");
            const color = COLORS[si % COLORS.length];
            return (
              <g key={series.key}>
                <path
                  d={`${path} L ${toX(sorted[sorted.length - 1].time)} ${toY(0)} L ${toX(sorted[0].time)} ${toY(0)} Z`}
                  fill={color}
                  fillOpacity={0.08}
                />
                <path
                  d={path}
                  fill="none"
                  stroke={color}
                  strokeWidth={2}
                  strokeLinejoin="round"
                  strokeLinecap="round"
                />
              </g>
            );
          })}
        </svg>
        {/* Legend */}
        <div
          style={{
            display: "flex",
            gap: 12,
            marginTop: 8,
            flexWrap: "wrap",
            fontSize: 12,
          }}
        >
          {timeseriesData.map((series, si) => (
            <div
              key={series.key}
              style={{ display: "flex", alignItems: "center", gap: 4 }}
            >
              <span
                style={{
                  width: 10,
                  height: 10,
                  borderRadius: 2,
                  background: COLORS[si % COLORS.length],
                }}
              />
              {series.key}
            </div>
          ))}
        </div>
      </div>
    );
  };

  return (
    <div style={{ maxWidth: 960, margin: "0 auto" }}>
      <div
        style={{
          display: "flex",
          justifyContent: "space-between",
          alignItems: "center",
          marginBottom: 16,
        }}
      >
        <h2 style={{ margin: 0 }}>Coding Activity</h2>
        <div style={{ display: "flex", gap: 6 }}>
          {["24h", "7d", "30d", "90d"].map((r) => (
            <button
              key={r}
              className={timeRange === r ? "primary" : ""}
              style={{ padding: "4px 12px", fontSize: 13 }}
              onClick={() => setTimeRange(r)}
            >
              {r}
            </button>
          ))}
        </div>
      </div>

      {/* Send Test Heartbeat */}
      <div className="card" style={{ marginBottom: 16, padding: 16 }}>
        <h4 style={{ marginTop: 0 }}>Send Test Heartbeat</h4>
        <textarea
          value={heartbeatInput}
          onChange={(e) => setHeartbeatInput(e.target.value)}
          rows={4}
          style={{
            width: "100%",
            marginBottom: 8,
            fontFamily: "monospace",
            fontSize: 13,
          }}
        />
        <button className="primary" onClick={sendHeartbeat}>
          Send Heartbeat
        </button>
        {error && (
          <p style={{ color: "var(--danger)", marginTop: 8 }}>{error}</p>
        )}
      </div>

      {loading ? (
        <p>Loading stats...</p>
      ) : (
        <>
          {/* Two-column leaderboards */}
          <div
            style={{
              display: "grid",
              gridTemplateColumns: "1fr 1fr",
              gap: 16,
              marginBottom: 16,
            }}
          >
            {renderLeaderboard(projectData, "Per Project")}
            {renderLeaderboard(languageData, "Per Language")}
          </div>

          {/* Time series */}
          {renderTimeseries()}

          {/* Per File */}
          <div style={{ marginTop: 16 }}>
            {renderLeaderboard(fileData, "Per File")}
          </div>
        </>
      )}
    </div>
  );
}
