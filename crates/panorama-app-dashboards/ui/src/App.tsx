// Dashboards Dashboard Plugin — Full dashboard viewer with panel renderers,
// time range picker, dashboard CRUD, and query builder.
//
// Panel types: leaderboard (bar), timeseries (line/area), stat (big number),
// piechart, table, heatmap (calendar grid).
//
// Charts are rendered with inline SVG — no external chart library dependency.

import {
  useState,
  useEffect,
  useCallback,
  useMemo,
  CSSProperties,
} from "react";

// ── API helper ────────────────────────────────────────────────────────────────

const PLUGIN_ID = "io.mzhang.panorama.dashboards";
const WAKA_PLUGIN = "io.mzhang.panorama.coding";

async function callPlugin(
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

// ── Types ─────────────────────────────────────────────────────────────────────

interface GridPos {
  x: number;
  y: number;
  w: number;
  h: number;
}

interface PanelQuery {
  ref_id: string;
  data_source: string;
  promql?: string;
  limit?: number;
  hide?: boolean;
}

interface Panel {
  id: number;
  title: string;
  type: string;
  grid_pos: GridPos;
  queries: PanelQuery[];
  options?: Record<string, unknown>;
  description?: string;
}

interface DashboardTime {
  from: string;
  to: string;
}

interface Dashboard {
  uid: string;
  title: string;
  description?: string;
  tags?: string[];
  time: DashboardTime;
  refresh?: string;
  panels: Panel[];
  variables?: any[];
}

interface DataFrame {
  name: string;
  columns: string[];
  rows: any[][];
  meta?: Record<string, unknown>;
}

interface TimeRangePreset {
  label: string;
  from: string;
  to: string;
}

// ── Color palette ─────────────────────────────────────────────────────────────

const PALETTE = [
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
  "#ff9966",
  "#66b3ff",
  "#ff66b3",
  "#b3ff66",
];

// ── Time Range Picker ─────────────────────────────────────────────────────────

const PRESETS: TimeRangePreset[] = [
  { label: "Last 1h", from: "now-1h", to: "now" },
  { label: "Last 3h", from: "now-3h", to: "now" },
  { label: "Last 6h", from: "now-6h", to: "now" },
  { label: "Last 12h", from: "now-12h", to: "now" },
  { label: "Last 24h", from: "now-24h", to: "now" },
  { label: "Last 2 days", from: "now-2d", to: "now" },
  { label: "Last 7 days", from: "now-7d", to: "now" },
  { label: "Last 30 days", from: "now-30d", to: "now" },
  { label: "Last 90 days", from: "now-90d", to: "now" },
  { label: "Last 1 year", from: "now-365d", to: "now" },
];

function TimeRangePicker({
  time,
  onChange,
}: {
  time: DashboardTime;
  onChange: (t: DashboardTime) => void;
}) {
  const [custom, setCustom] = useState(false);

  const activePreset = PRESETS.find(
    (p) => p.from === time.from && p.to === time.to,
  );

  return (
    <div
      style={{
        display: "flex",
        gap: 6,
        alignItems: "center",
        flexWrap: "wrap",
      }}
    >
      {PRESETS.slice(0, 6).map((p) => (
        <button
          key={p.label}
          className={activePreset?.label === p.label ? "primary" : ""}
          style={{ padding: "4px 10px", fontSize: 12 }}
          onClick={() => onChange({ from: p.from, to: p.to })}
        >
          {p.label}
        </button>
      ))}
      <button
        style={{ padding: "4px 10px", fontSize: 12 }}
        onClick={() => setCustom(!custom)}
      >
        Custom...
      </button>
      {custom && (
        <div style={{ display: "flex", gap: 4, alignItems: "center" }}>
          <input
            type="text"
            placeholder="now-7d"
            value={time.from}
            onChange={(e) => onChange({ ...time, from: e.target.value })}
            style={{ width: 100, padding: "3px 6px", fontSize: 12 }}
          />
          <span className="text-muted">to</span>
          <input
            type="text"
            placeholder="now"
            value={time.to}
            onChange={(e) => onChange({ ...time, to: e.target.value })}
            style={{ width: 100, padding: "3px 6px", fontSize: 12 }}
          />
        </div>
      )}
    </div>
  );
}

// ── Panel Renderers ───────────────────────────────────────────────────────────

/** Leaderboard: horizontal bar chart */
function LeaderboardPanel({
  data,
  options,
}: {
  data: DataFrame[];
  options?: Record<string, unknown>;
}) {
  const rows = data[0]?.rows ?? [];
  // rows: [[key, seconds, hours], ...]
  const maxHours = rows[0]?.[2] ?? 1;
  const limit = (options?.limit as number) ?? 10;
  const displayed = rows.slice(0, limit);

  if (!displayed.length) {
    return (
      <p className="text-muted" style={{ padding: 16, textAlign: "center" }}>
        No data
      </p>
    );
  }

  return (
    <div style={{ padding: "8px 0" }}>
      {displayed.map((row: any[], i: number) => {
        const key = String(row[0] ?? "(unknown)");
        const hours = Number(row[2] ?? 0);
        const pct = maxHours > 0 ? (hours / maxHours) * 100 : 0;

        return (
          <div key={key} style={{ marginBottom: 4 }}>
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
                {key}
              </span>
              <span
                className="text-muted"
                style={{ fontFamily: "monospace", fontSize: 12 }}
              >
                {formatHours(hours)}
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
                  background: PALETTE[i % PALETTE.length],
                  borderRadius: 3,
                  transition: "width 0.5s ease",
                }}
              />
            </div>
          </div>
        );
      })}
    </div>
  );
}

/** Timeseries: SVG line/area chart */
function TimeseriesPanel({
  data,
  options,
}: {
  data: DataFrame[];
  options?: Record<string, unknown>;
}) {
  const W = 700,
    H = 240,
    PAD = { top: 16, right: 16, bottom: 32, left: 56 };
  const plotW = W - PAD.left - PAD.right;
  const plotH = H - PAD.top - PAD.bottom;

  // Collect all series
  const allSeries: { key: string; points: { time: number; val: number }[] }[] =
    [];
  for (const frame of data) {
    if (!frame.rows.length) continue;
    // timeseries frames: columns = [time, iso, ...series_keys]
    // or simple frame: columns = [key, seconds, hours]  — not timeseries
    const cols = frame.columns;
    if (cols[0] === "time") {
      // Multi-series timeseries frame
      for (let ci = 2; ci < cols.length; ci++) {
        const key = cols[ci];
        const points = frame.rows.map((r) => ({
          time: Number(r[0]),
          val: Number(r[ci] ?? 0),
        }));
        allSeries.push({ key, points });
      }
    }
  }

  if (!allSeries.length) {
    return (
      <p className="text-muted" style={{ padding: 16, textAlign: "center" }}>
        No time-series data
      </p>
    );
  }

  // Determine time range
  const allTimes = allSeries.flatMap((s) => s.points.map((p) => p.time));
  const tMin = Math.min(...allTimes);
  const tMax = Math.max(...allTimes);
  const tRange = tMax - tMin || 1;

  // Determine value range
  const allVals = allSeries.flatMap((s) => s.points.map((p) => p.val));
  const vMin = 0;
  const vMax = Math.max(...allVals) * 1.1 || 1;
  const vRange = vMax - vMin || 1;

  const toX = (t: number) => PAD.left + ((t - tMin) / tRange) * plotW;
  const toY = (v: number) => PAD.top + plotH - ((v - vMin) / vRange) * plotH;

  // Y-axis ticks
  const yTicks = 4;
  const yTickVals = Array.from(
    { length: yTicks + 1 },
    (_, i) => vMin + (vRange * i) / yTicks,
  );

  // X-axis ticks (dates)
  const xTickCount = Math.min(7, allTimes.length);
  const xTickVals = Array.from({ length: xTickCount }, (_, i) => {
    const idx = Math.floor((i / (xTickCount - 1 || 1)) * (allTimes.length - 1));
    return allTimes[Math.min(idx, allTimes.length - 1)];
  });

  return (
    <svg
      viewBox={`0 0 ${W} ${H}`}
      style={{ width: "100%", height: "auto", minHeight: 200 }}
    >
      {/* Grid lines */}
      {yTickVals.map((v, i) => (
        <g key={`y-${i}`}>
          <line
            x1={PAD.left}
            y1={toY(v)}
            x2={W - PAD.right}
            y2={toY(v)}
            stroke="var(--border)"
            strokeWidth={0.5}
            strokeDasharray="4 2"
          />
          <text
            x={PAD.left - 6}
            y={toY(v) + 4}
            textAnchor="end"
            fill="var(--text-muted)"
            fontSize={10}
          >
            {formatHours(v)}
          </text>
        </g>
      ))}
      {/* X-axis labels */}
      {xTickVals.map((t, i) => (
        <text
          key={`x-${i}`}
          x={toX(t)}
          y={H - 6}
          textAnchor="middle"
          fill="var(--text-muted)"
          fontSize={10}
        >
          {fmtTimeLabel(t, tRange)}
        </text>
      ))}
      {/* Data lines */}
      {allSeries.map((series, si) => {
        const sorted = [...series.points].sort((a, b) => a.time - b.time);
        if (sorted.length < 2) return null;
        const pathD = sorted
          .map((p, i) => `${i === 0 ? "M" : "L"} ${toX(p.time)} ${toY(p.val)}`)
          .join(" ");
        const areaD =
          pathD +
          ` L ${toX(sorted[sorted.length - 1].time)} ${toY(0)} L ${toX(sorted[0].time)} ${toY(0)} Z`;
        const color = PALETTE[si % PALETTE.length];
        return (
          <g key={series.key}>
            {options?.fill !== 0 && (
              <path d={areaD} fill={color} fillOpacity={0.1} />
            )}
            <path
              d={pathD}
              fill="none"
              stroke={color}
              strokeWidth={(options?.lineWidth as number) ?? 2}
              strokeLinejoin="round"
              strokeLinecap="round"
            />
          </g>
        );
      })}
    </svg>
  );
}

/** Stat panel: single big number with label */
function StatPanel({
  data,
  options,
}: {
  data: DataFrame[];
  options?: Record<string, unknown>;
}) {
  const rows = data[0]?.rows;
  const val = rows?.[0]?.[0];
  const label = rows?.[0]?.[1] ?? (options?.label as string) ?? "";

  return (
    <div style={{ textAlign: "center", padding: "24px 16px" }}>
      <div style={{ fontSize: 48, fontWeight: 700, lineHeight: 1.1 }}>
        {val != null ? String(val) : "—"}
      </div>
      {label && (
        <div className="text-muted" style={{ marginTop: 8, fontSize: 14 }}>
          {String(label)}
        </div>
      )}
    </div>
  );
}

/** Pie chart: SVG donut */
function PieChartPanel({
  data,
  options,
}: {
  data: DataFrame[];
  options?: Record<string, unknown>;
}) {
  const rows = data[0]?.rows ?? [];
  // rows: [[key, seconds, percent], ...]
  if (!rows.length) {
    return (
      <p className="text-muted" style={{ padding: 16, textAlign: "center" }}>
        No data
      </p>
    );
  }

  const R = 80,
    CX = 100,
    CY = 100;
  let cumAngle = -Math.PI / 2; // Start from top
  const total = rows.reduce((s: number, r: any[]) => s + Number(r[1] ?? 0), 0);

  const donut = options?.donut !== false;
  const innerR = donut ? R * 0.55 : 0;

  return (
    <div style={{ display: "flex", alignItems: "center", gap: 16, padding: 8 }}>
      <svg
        viewBox="0 0 200 200"
        style={{ width: 140, height: 140, flexShrink: 0 }}
      >
        {rows.map((row: any[], i: number) => {
          const pct = total > 0 ? Number(row[1] ?? 0) / total : 0;
          const angle = pct * 2 * Math.PI;
          const startAngle = cumAngle;
          const endAngle = cumAngle + angle;
          cumAngle = endAngle;

          const x1 = CX + R * Math.cos(startAngle);
          const y1 = CY + R * Math.sin(startAngle);
          const x2 = CX + R * Math.cos(endAngle);
          const y2 = CY + R * Math.sin(endAngle);
          const largeArc = angle > Math.PI ? 1 : 0;

          const ix1 = CX + innerR * Math.cos(startAngle);
          const iy1 = CY + innerR * Math.sin(startAngle);
          const ix2 = CX + innerR * Math.cos(endAngle);
          const iy2 = CY + innerR * Math.sin(endAngle);

          const d = donut
            ? `M ${x1} ${y1} A ${R} ${R} 0 ${largeArc} 1 ${x2} ${y2} L ${ix2} ${iy2} A ${innerR} ${innerR} 0 ${largeArc} 0 ${ix1} ${iy1} Z`
            : `M ${CX} ${CY} L ${x1} ${y1} A ${R} ${R} 0 ${largeArc} 1 ${x2} ${y2} Z`;

          return (
            <path
              key={i}
              d={d}
              fill={PALETTE[i % PALETTE.length]}
              stroke="var(--bg-card)"
              strokeWidth={1}
            />
          );
        })}
      </svg>
      <div style={{ flex: 1, minWidth: 0 }}>
        {rows.slice(0, 8).map((row: any[], i: number) => (
          <div
            key={i}
            style={{
              display: "flex",
              alignItems: "center",
              gap: 6,
              marginBottom: 3,
              fontSize: 12,
            }}
          >
            <span
              style={{
                width: 10,
                height: 10,
                borderRadius: 2,
                background: PALETTE[i % PALETTE.length],
                flexShrink: 0,
              }}
            />
            <span
              style={{
                overflow: "hidden",
                textOverflow: "ellipsis",
                whiteSpace: "nowrap",
              }}
            >
              {String(row[0] ?? "(unknown)")}
            </span>
            <span
              className="text-muted"
              style={{
                marginLeft: "auto",
                fontFamily: "monospace",
                fontSize: 11,
              }}
            >
              {row[2] != null ? `${Number(row[2]).toFixed(0)}%` : ""}
            </span>
          </div>
        ))}
      </div>
    </div>
  );
}

/** Table panel */
function TablePanel({
  data,
  options,
}: {
  data: DataFrame[];
  options?: Record<string, unknown>;
}) {
  const frame = data[0];
  if (!frame?.rows.length) {
    return (
      <p className="text-muted" style={{ padding: 16, textAlign: "center" }}>
        No data
      </p>
    );
  }

  return (
    <div style={{ overflow: "auto", maxHeight: 400 }}>
      <table
        style={{ width: "100%", borderCollapse: "collapse", fontSize: 13 }}
      >
        <thead>
          <tr>
            <th style={thStyle}>#</th>
            {frame.columns.map((col: string) => (
              <th key={col} style={thStyle}>
                {col}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {frame.rows.map((row: any[], i: number) => (
            <tr key={i} style={{ borderBottom: "1px solid var(--border)" }}>
              <td style={tdStyle} className="text-muted">
                {i + 1}
              </td>
              {row.map((cell: any, j: number) => (
                <td key={j} style={tdStyle}>
                  {typeof cell === "number"
                    ? cell.toLocaleString()
                    : String(cell)}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

const thStyle: CSSProperties = {
  textAlign: "left",
  padding: "6px 8px",
  borderBottom: "1px solid var(--border)",
  fontWeight: 600,
  position: "sticky",
  top: 0,
  background: "var(--bg-card)",
};
const tdStyle: CSSProperties = { padding: "4px 8px", whiteSpace: "nowrap" };

/** Heatmap: calendar-style grid */
function HeatmapPanel({ data }: { data: DataFrame[] }) {
  const rows = data[0]?.rows ?? [];
  // rows: [[date, seconds, hours], ...]

  if (!rows.length) {
    return (
      <p className="text-muted" style={{ padding: 16, textAlign: "center" }}>
        No data
      </p>
    );
  }

  const maxHours = Math.max(...rows.map((r: any[]) => Number(r[2] ?? 0)), 0.01);
  const dayMap = new Map(
    rows.map((r: any[]) => [String(r[0]), Number(r[2] ?? 0)]),
  );

  // Build 52-week grid
  const now = new Date();
  const weeks: { date: string; hours: number }[][] = [];
  const totalWeeks = 26; // Show last 26 weeks

  for (let w = totalWeeks - 1; w >= 0; w--) {
    const week: { date: string; hours: number }[] = [];
    for (let d = 6; d >= 0; d--) {
      const date = new Date(now);
      date.setDate(date.getDate() - (w * 7 + d));
      const dateStr = date.toISOString().slice(0, 10);
      week.push({ date: dateStr, hours: dayMap.get(dateStr) ?? 0 });
    }
    weeks.push(week);
  }

  const getIntensity = (hours: number) => {
    if (hours <= 0) return "var(--bg)";
    const pct = hours / maxHours;
    if (pct < 0.25) return "#9be9a8";
    if (pct < 0.5) return "#40c463";
    if (pct < 0.75) return "#30a14e";
    return "#216e39";
  };

  const dayNames = ["", "Mon", "", "Wed", "", "Fri", ""];

  return (
    <div style={{ padding: 8, overflow: "auto" }}>
      <div style={{ display: "flex", gap: 3 }}>
        {/* Day labels */}
        <div
          style={{
            display: "flex",
            flexDirection: "column",
            gap: 3,
            marginRight: 4,
          }}
        >
          {dayNames.map((name, i) => (
            <div
              key={i}
              style={{
                width: 24,
                height: 14,
                fontSize: 9,
                lineHeight: "14px",
                textAlign: "right",
              }}
              className="text-muted"
            >
              {name}
            </div>
          ))}
        </div>
        {/* Weeks */}
        {weeks.map((week, wi) => (
          <div
            key={wi}
            style={{ display: "flex", flexDirection: "column", gap: 3 }}
          >
            {week.map((day, di) => (
              <div
                key={di}
                title={`${day.date}: ${formatHours(day.hours)}`}
                style={{
                  width: 14,
                  height: 14,
                  borderRadius: 2,
                  background: getIntensity(day.hours),
                }}
              />
            ))}
          </div>
        ))}
      </div>
      <div
        style={{
          display: "flex",
          gap: 4,
          alignItems: "center",
          marginTop: 8,
          fontSize: 10,
        }}
        className="text-muted"
      >
        <span>Less</span>
        {[0, 0.25, 0.5, 0.75, 1].map((pct) => (
          <div
            key={pct}
            style={{
              width: 12,
              height: 12,
              borderRadius: 2,
              background: getIntensity(maxHours * pct),
            }}
          />
        ))}
        <span>More</span>
      </div>
    </div>
  );
}

// ── Panel Switcher ────────────────────────────────────────────────────────────

function PanelRenderer({ panel, data }: { panel: Panel; data: DataFrame[] }) {
  switch (panel.type) {
    case "leaderboard":
      return <LeaderboardPanel data={data} options={panel.options} />;
    case "timeseries":
      return <TimeseriesPanel data={data} options={panel.options} />;
    case "stat":
      return <StatPanel data={data} options={panel.options} />;
    case "piechart":
      return <PieChartPanel data={data} options={panel.options} />;
    case "table":
      return <TablePanel data={data} options={panel.options} />;
    case "heatmap":
      return <HeatmapPanel data={data} />;
    default:
      return <LeaderboardPanel data={data} options={panel.options} />;
  }
}

// ── Main Dashboard View ───────────────────────────────────────────────────────

export default function DashboardsApp({ pluginId }: { pluginId: string }) {
  const pid = pluginId || PLUGIN_ID;
  const [dashboards, setDashboards] = useState<any[]>([]);
  const [currentDashboard, setCurrentDashboard] = useState<Dashboard | null>(
    null,
  );
  const [panelData, setPanelData] = useState<Map<number, DataFrame[]>>(
    new Map(),
  );
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [showBuilder, setShowBuilder] = useState(false);
  const [editingDashboard, setEditingDashboard] = useState<Dashboard | null>(
    null,
  );
  const [time, setTime] = useState<DashboardTime>({
    from: "now-7d",
    to: "now",
  });
  const [autoRefresh, setAutoRefresh] = useState("");

  // Load dashboard list
  const loadDashboards = useCallback(async () => {
    try {
      const res = await callPlugin(pid, "api/dashboards");
      const list = await res.json();
      setDashboards(Array.isArray(list) ? list : []);
    } catch (e) {
      /* ignore */
    }
  }, [pid]);

  // Load a specific dashboard
  const loadDashboard = useCallback(
    async (uid: string) => {
      setLoading(true);
      setError("");
      try {
        const res = await callPlugin(pid, `api/dashboards/${uid}`);
        if (!res.ok) throw new Error(`Dashboard not found: ${uid}`);
        const dash: Dashboard = await res.json();
        setCurrentDashboard(dash);
        if (dash.time) setTime(dash.time);
        if (dash.refresh) setAutoRefresh(dash.refresh);
        // Fetch data for each panel
        await fetchPanelData(dash);
      } catch (e: any) {
        setError(e.message);
        // If no dashboards exist, try loading home
        if (!currentDashboard) {
          try {
            const homeRes = await callPlugin(pid, "api/dashboards/home");
            const home: Dashboard = await homeRes.json();
            setCurrentDashboard(home);
            if (home.time) setTime(home.time);
            await fetchPanelData(home);
            setError("");
          } catch {
            /* ignore */
          }
        }
      }
      setLoading(false);
    },
    [pid],
  );

  // Fetch data for all panels
  const fetchPanelData = async (dash: Dashboard) => {
    const newData = new Map<number, DataFrame[]>();
    for (const panel of dash.panels) {
      if (!panel.queries?.length) continue;
      try {
        const res = await callPlugin(pid, "api/ds/query", "POST", {
          queries: panel.queries,
          range: dash.time ?? time,
        });
        const frames: DataFrame[] = await res.json();
        newData.set(panel.id, frames);
      } catch {
        /* panel will show "no data" */
      }
    }
    setPanelData(newData);
  };

  // Initial load
  useEffect(() => {
    loadDashboards();
    // Try loading first dashboard or home
    callPlugin(pid, "api/dashboards/home")
      .then((res) => res.json())
      .then((home: Dashboard) => {
        setCurrentDashboard(home);
        if (home.time) setTime(home.time);
        return fetchPanelData(home);
      })
      .catch(() => setError("No dashboards yet. Create one!"))
      .finally(() => setLoading(false));
  }, [pid]);

  // Auto-refresh
  useEffect(() => {
    if (!autoRefresh || !currentDashboard) return;
    const secs = parseRefreshInterval(autoRefresh);
    if (!secs) return;
    const timer = setInterval(
      () => fetchPanelData(currentDashboard),
      secs * 1000,
    );
    return () => clearInterval(timer);
  }, [autoRefresh, currentDashboard]);

  // Refresh data when time range changes
  useEffect(() => {
    if (currentDashboard) {
      const updated = { ...currentDashboard, time };
      fetchPanelData(updated);
    }
  }, [time.from, time.to]);

  const handleSaveDashboard = async (dash: Dashboard) => {
    try {
      const method = dash.uid ? "PUT" : "POST";
      const endpoint = dash.uid
        ? `api/dashboards/${dash.uid}`
        : "api/dashboards";
      const res = await callPlugin(pid, endpoint, method, dash);
      if (!res.ok) throw new Error("Failed to save");
      const saved = await res.json();
      setShowBuilder(false);
      setEditingDashboard(null);
      loadDashboards();
      loadDashboard(saved.uid ?? dash.uid);
    } catch (e: any) {
      setError(e.message);
    }
  };

  const handleDeleteDashboard = async (uid: string) => {
    if (!confirm("Delete this dashboard?")) return;
    try {
      await callPlugin(pid, `api/dashboards/${uid}`, "DELETE");
      setCurrentDashboard(null);
      loadDashboards();
    } catch (e: any) {
      setError(e.message);
    }
  };

  return (
    <div style={{ maxWidth: 1200, margin: "0 auto", padding: "0 16px" }}>
      {/* Header */}
      <div
        style={{
          display: "flex",
          justifyContent: "space-between",
          alignItems: "center",
          marginBottom: 16,
          flexWrap: "wrap",
          gap: 8,
        }}
      >
        <h2 style={{ margin: 0 }}>{currentDashboard?.title ?? "Dashboards"}</h2>
        <div
          style={{
            display: "flex",
            gap: 8,
            alignItems: "center",
            flexWrap: "wrap",
          }}
        >
          <TimeRangePicker time={time} onChange={setTime} />
          <select
            value={autoRefresh}
            onChange={(e) => setAutoRefresh(e.target.value)}
            style={{ padding: "4px 8px", fontSize: 12 }}
          >
            <option value="">Off</option>
            <option value="30s">30s</option>
            <option value="1m">1m</option>
            <option value="5m">5m</option>
            <option value="15m">15m</option>
            <option value="30m">30m</option>
            <option value="1h">1h</option>
          </select>
          <button
            className="primary"
            onClick={() => {
              setEditingDashboard(
                currentDashboard
                  ? { ...currentDashboard }
                  : {
                      uid: "",
                      title: "New Dashboard",
                      time: { from: "now-7d", to: "now" },
                      panels: [],
                    },
              );
              setShowBuilder(true);
            }}
          >
            {currentDashboard ? "Edit" : "New Dashboard"}
          </button>
        </div>
      </div>

      {error && (
        <div
          className="card"
          style={{ marginBottom: 16, color: "var(--danger)", padding: 12 }}
        >
          {error}
          <button
            style={{ marginLeft: 12, fontSize: 12 }}
            onClick={() => setError("")}
          >
            Dismiss
          </button>
        </div>
      )}

      {/* Dashboard Selector */}
      {dashboards.length > 1 && (
        <div
          style={{
            marginBottom: 16,
            display: "flex",
            gap: 6,
            flexWrap: "wrap",
          }}
        >
          {dashboards.map((d: any) => (
            <button
              key={d.uid}
              className={d.uid === currentDashboard?.uid ? "primary" : ""}
              style={{ padding: "4px 12px", fontSize: 13 }}
              onClick={() => loadDashboard(d.uid)}
            >
              {d.title}
            </button>
          ))}
        </div>
      )}

      {/* Panel Grid */}
      {loading ? (
        <p>Loading dashboard...</p>
      ) : currentDashboard ? (
        <div
          style={{
            display: "grid",
            gridTemplateColumns: "repeat(24, 1fr)",
            gap: 12,
          }}
        >
          {currentDashboard.panels.map((panel) => {
            const data = panelData.get(panel.id) ?? [];
            const pos = panel.grid_pos ?? { x: 0, y: 0, w: 12, h: 8 };
            return (
              <div
                key={panel.id}
                className="card"
                style={{
                  gridColumn: `${pos.x + 1} / span ${pos.w}`,
                  gridRow: `${pos.y + 1} / span ${pos.h}`,
                  minHeight: Math.max(pos.h * 30, 120),
                  display: "flex",
                  flexDirection: "column",
                  overflow: "hidden",
                }}
              >
                <div
                  style={{
                    display: "flex",
                    justifyContent: "space-between",
                    alignItems: "center",
                    marginBottom: 8,
                  }}
                >
                  <h4 style={{ margin: 0 }}>{panel.title}</h4>
                  {panel.description && (
                    <span
                      className="text-muted"
                      style={{ fontSize: 11 }}
                      title={panel.description}
                    >
                      ⓘ
                    </span>
                  )}
                </div>
                <div style={{ flex: 1, minHeight: 0 }}>
                  <PanelRenderer panel={panel} data={data} />
                </div>
              </div>
            );
          })}
          {currentDashboard.panels.length === 0 && (
            <div
              style={{ gridColumn: "1 / -1", textAlign: "center", padding: 48 }}
              className="text-muted"
            >
              <p>This dashboard has no panels.</p>
              <button
                className="primary"
                onClick={() => {
                  setEditingDashboard({ ...currentDashboard });
                  setShowBuilder(true);
                }}
              >
                Add Panels
              </button>
            </div>
          )}
        </div>
      ) : null}

      {/* Builder Modal */}
      {showBuilder && editingDashboard && (
        <DashboardBuilder
          dashboard={editingDashboard}
          onChange={setEditingDashboard}
          onSave={handleSaveDashboard}
          onCancel={() => {
            setShowBuilder(false);
            setEditingDashboard(null);
          }}
          onDelete={
            editingDashboard.uid
              ? () => handleDeleteDashboard(editingDashboard.uid)
              : undefined
          }
        />
      )}
    </div>
  );
}

// ── Dashboard Builder / Editor ─────────────────────────────────────────────────

const PANEL_TYPES = [
  { value: "leaderboard", label: "Leaderboard" },
  { value: "timeseries", label: "Time Series" },
  { value: "stat", label: "Single Stat" },
  { value: "piechart", label: "Pie Chart" },
  { value: "table", label: "Table" },
  { value: "heatmap", label: "Heatmap" },
];

function DashboardBuilder({
  dashboard,
  onChange,
  onSave,
  onCancel,
  onDelete,
}: {
  dashboard: Dashboard;
  onChange: (d: Dashboard) => void;
  onSave: (d: Dashboard) => void;
  onCancel: () => void;
  onDelete?: () => void;
}) {
  const handleAddPanel = () => {
    const newId = Math.max(0, ...dashboard.panels.map((p) => p.id)) + 1;
    const panelCount = dashboard.panels.length;
    const newPanel: Panel = {
      id: newId,
      title: `Panel ${newId}`,
      type: "leaderboard",
      grid_pos: { x: 0, y: panelCount * 8, w: 12, h: 8 },
      queries: [
        {
          ref_id: "A",
          data_source: WAKA_PLUGIN,
          promql: "sum by (project) (coding_duration)",
          limit: 10,
        },
      ],
    };
    onChange({ ...dashboard, panels: [...dashboard.panels, newPanel] });
  };

  const handleUpdatePanel = (id: number, updates: Partial<Panel>) => {
    onChange({
      ...dashboard,
      panels: dashboard.panels.map((p) =>
        p.id === id ? { ...p, ...updates } : p,
      ),
    });
  };

  const handleRemovePanel = (id: number) => {
    onChange({
      ...dashboard,
      panels: dashboard.panels.filter((p) => p.id !== id),
    });
  };

  return (
    <div
      style={{
        position: "fixed",
        inset: 0,
        background: "rgba(0,0,0,0.5)",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        zIndex: 1000,
      }}
    >
      <div
        className="card"
        style={{
          width: "90vw",
          maxWidth: 800,
          maxHeight: "90vh",
          overflow: "auto",
          padding: 24,
        }}
      >
        <h3 style={{ marginTop: 0 }}>
          {dashboard.uid ? "Edit Dashboard" : "New Dashboard"}
        </h3>

        {/* Dashboard meta */}
        <div
          style={{
            display: "flex",
            gap: 12,
            marginBottom: 16,
            flexWrap: "wrap",
          }}
        >
          <div style={{ flex: 1, minWidth: 200 }}>
            <label
              className="text-muted"
              style={{ display: "block", marginBottom: 4 }}
            >
              Title
            </label>
            <input
              type="text"
              value={dashboard.title}
              onChange={(e) =>
                onChange({ ...dashboard, title: e.target.value })
              }
              style={{ width: "100%" }}
            />
          </div>
          <div style={{ flex: 1, minWidth: 200 }}>
            <label
              className="text-muted"
              style={{ display: "block", marginBottom: 4 }}
            >
              Description
            </label>
            <input
              type="text"
              value={dashboard.description ?? ""}
              onChange={(e) =>
                onChange({ ...dashboard, description: e.target.value })
              }
              style={{ width: "100%" }}
            />
          </div>
        </div>

        <div
          style={{
            display: "flex",
            gap: 12,
            marginBottom: 16,
            flexWrap: "wrap",
          }}
        >
          <div>
            <label
              className="text-muted"
              style={{ display: "block", marginBottom: 4 }}
            >
              Time From
            </label>
            <input
              type="text"
              value={dashboard.time?.from ?? "now-7d"}
              onChange={(e) =>
                onChange({
                  ...dashboard,
                  time: { ...dashboard.time, from: e.target.value },
                })
              }
              style={{ width: 120 }}
            />
          </div>
          <div>
            <label
              className="text-muted"
              style={{ display: "block", marginBottom: 4 }}
            >
              Time To
            </label>
            <input
              type="text"
              value={dashboard.time?.to ?? "now"}
              onChange={(e) =>
                onChange({
                  ...dashboard,
                  time: { ...dashboard.time, to: e.target.value },
                })
              }
              style={{ width: 120 }}
            />
          </div>
          <div>
            <label
              className="text-muted"
              style={{ display: "block", marginBottom: 4 }}
            >
              Refresh
            </label>
            <select
              value={dashboard.refresh ?? ""}
              onChange={(e) =>
                onChange({ ...dashboard, refresh: e.target.value })
              }
            >
              <option value="">Off</option>
              <option value="30s">30s</option>
              <option value="1m">1m</option>
              <option value="5m">5m</option>
              <option value="15m">15m</option>
              <option value="30m">30m</option>
              <option value="1h">1h</option>
            </select>
          </div>
        </div>

        <div>
          <div
            style={{
              display: "flex",
              justifyContent: "space-between",
              alignItems: "center",
              marginBottom: 12,
            }}
          >
            <h4 style={{ margin: 0 }}>Panels ({dashboard.panels.length})</h4>
            <button onClick={handleAddPanel}>+ Add Panel</button>
          </div>

          {dashboard.panels.map((panel, idx) => (
            <div
              key={panel.id}
              className="card"
              style={{ marginBottom: 12, padding: 12, background: "var(--bg)" }}
            >
              <div
                style={{
                  display: "flex",
                  justifyContent: "space-between",
                  alignItems: "center",
                  marginBottom: 8,
                }}
              >
                <strong>
                  Panel {panel.id}: {panel.title}
                </strong>
                <button
                  style={{
                    color: "var(--danger)",
                    fontSize: 12,
                    padding: "2px 8px",
                  }}
                  onClick={() => handleRemovePanel(panel.id)}
                >
                  Remove
                </button>
              </div>

              <div
                style={{
                  display: "grid",
                  gridTemplateColumns: "1fr 1fr",
                  gap: 8,
                }}
              >
                <div>
                  <label
                    className="text-muted"
                    style={{ display: "block", fontSize: 11 }}
                  >
                    Title
                  </label>
                  <input
                    type="text"
                    value={panel.title}
                    onChange={(e) =>
                      handleUpdatePanel(panel.id, { title: e.target.value })
                    }
                    style={{ width: "100%", fontSize: 13, padding: "4px 8px" }}
                  />
                </div>
                <div>
                  <label
                    className="text-muted"
                    style={{ display: "block", fontSize: 11 }}
                  >
                    Type
                  </label>
                  <select
                    value={panel.type}
                    onChange={(e) =>
                      handleUpdatePanel(panel.id, { type: e.target.value })
                    }
                  >
                    {PANEL_TYPES.map((o) => (
                      <option key={o.value} value={o.value}>
                        {o.label}
                      </option>
                    ))}
                  </select>
                </div>
              </div>

              {/* Panel query editor */}
              {panel.queries.map((q, qi) => (
                <div
                  key={qi}
                  style={{
                    marginTop: 8,
                    padding: "8px",
                    background: "var(--bg-card)",
                    borderRadius: 4,
                  }}
                >
                  <div
                    style={{
                      display: "flex",
                      gap: 8,
                      flexWrap: "wrap",
                      alignItems: "end",
                    }}
                  >
                    <div style={{ minWidth: 100 }}>
                      <label
                        className="text-muted"
                        style={{ display: "block", fontSize: 11 }}
                      >
                        Data Source
                      </label>
                      <input
                        type="text"
                        value={q.data_source}
                        onChange={(e) => {
                          const queries = [...panel.queries];
                          queries[qi] = {
                            ...queries[qi],
                            data_source: e.target.value,
                          };
                          handleUpdatePanel(panel.id, { queries });
                        }}
                        style={{
                          width: "100%",
                          fontSize: 12,
                          padding: "3px 6px",
                        }}
                      />
                    </div>
                    <div>
                      <label
                        className="text-muted"
                        style={{ display: "block", fontSize: 11 }}
                      >
                        Limit
                      </label>
                      <input
                        type="number"
                        value={q.limit ?? 10}
                        onChange={(e) => {
                          const queries = [...panel.queries];
                          queries[qi] = {
                            ...queries[qi],
                            limit: parseInt(e.target.value) || 10,
                          };
                          handleUpdatePanel(panel.id, { queries });
                        }}
                        style={{ width: 60, fontSize: 12, padding: "3px 6px" }}
                      />
                    </div>
                  </div>
                  <div style={{ marginTop: 6 }}>
                    <label
                      className="text-muted"
                      style={{ display: "block", fontSize: 11 }}
                    >
                      PromQL Expression
                    </label>
                    <textarea
                      value={q.promql ?? ""}
                      placeholder={`coding_duration\nsum by (project) (coding_duration)\nrate(coding_duration[5m])\ntopk(10, sum by (language) (coding_duration))`}
                      rows={4}
                      onChange={(e) => {
                        const queries = [...panel.queries];
                        queries[qi] = {
                          ...queries[qi],
                          promql: e.target.value,
                        };
                        handleUpdatePanel(panel.id, { queries });
                      }}
                      style={{
                        width: "100%",
                        fontSize: 13,
                        padding: "6px 8px",
                        fontFamily: "monospace",
                        resize: "vertical",
                      }}
                    />
                  </div>
                </div>
              ))}

              {/* Grid position */}
              <details style={{ marginTop: 8, fontSize: 12 }}>
                <summary className="text-muted" style={{ cursor: "pointer" }}>
                  Grid Position
                </summary>
                <div style={{ display: "flex", gap: 8, marginTop: 8 }}>
                  {(["x", "y", "w", "h"] as const).map((k) => (
                    <div key={k}>
                      <label
                        className="text-muted"
                        style={{ display: "block", fontSize: 10 }}
                      >
                        {k}
                      </label>
                      <input
                        type="number"
                        value={
                          panel.grid_pos?.[k] ??
                          (k === "w" ? 12 : k === "h" ? 8 : 0)
                        }
                        onChange={(e) => {
                          const gp = {
                            ...panel.grid_pos,
                            [k]: parseInt(e.target.value) || 0,
                          };
                          handleUpdatePanel(panel.id, { grid_pos: gp });
                        }}
                        style={{ width: 50, fontSize: 12, padding: "2px 4px" }}
                      />
                    </div>
                  ))}
                </div>
              </details>
            </div>
          ))}
        </div>

        <div
          style={{
            display: "flex",
            gap: 8,
            marginTop: 20,
            justifyContent: "flex-end",
          }}
        >
          {onDelete && (
            <button
              style={{ color: "var(--danger)", marginRight: "auto" }}
              onClick={onDelete}
            >
              Delete Dashboard
            </button>
          )}
          <button onClick={onCancel}>Cancel</button>
          <button className="primary" onClick={() => onSave(dashboard)}>
            {dashboard.uid ? "Save Changes" : "Create Dashboard"}
          </button>
        </div>
      </div>
    </div>
  );
}

// ── Helpers ────────────────────────────────────────────────────────────────────

function formatHours(seconds: number): string {
  const hours = seconds / 3600;
  if (hours >= 1) return `${hours.toFixed(1)}h`;
  if (hours >= 0.01) return `${(hours * 60).toFixed(0)}m`;
  return `${seconds.toFixed(0)}s`;
}

function fmtTimeLabel(ts: number, rangeSec: number): string {
  const d = new Date(ts * 1000);
  if (rangeSec <= 86400) {
    return d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  }
  if (rangeSec <= 604800 * 4) {
    return `${d.getMonth() + 1}/${d.getDate()}`;
  }
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}`;
}

function parseRefreshInterval(refresh: string): number {
  if (!refresh) return 0;
  const match = refresh.match(/^(\d+)(s|m|h)$/);
  if (!match) return 0;
  const val = parseInt(match[1]);
  switch (match[2]) {
    case "s":
      return val;
    case "m":
      return val * 60;
    case "h":
      return val * 3600;
    default:
      return 0;
  }
}
