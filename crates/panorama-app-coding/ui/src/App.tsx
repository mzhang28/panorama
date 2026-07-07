import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";

interface SummariesEntry {
  name: string;
  total_seconds: number;
  percent: number;
  text: string;
  hours: number;
  minutes: number;
}

interface WakaTimeStats {
  data: {
    total_seconds: number;
    daily_average: number;
    human_readable_total: string;
    human_readable_daily_average: string;
    best_day?: { date: string; total_seconds: number; text: string };
    projects: SummariesEntry[];
    languages: SummariesEntry[];
    editors: SummariesEntry[];
    operating_systems: SummariesEntry[];
    machines: SummariesEntry[];
    categories: SummariesEntry[];
  };
}

interface TimeseriesSeries {
  key: string;
  bucket: string;
  data: { time: number; iso: string; seconds: number; hours: number }[];
}

interface CodingAppProps {
  pluginId: string;
}

const COLORS = [
  "#6366f1",
  "#8b5cf6",
  "#a855f7",
  "#d946ef",
  "#ec4899",
  "#f43f5e",
  "#ef4444",
  "#f97316",
  "#eab308",
  "#22c55e",
  "#14b8a6",
  "#06b6d4",
];

async function callPlugin(
  pluginId: string,
  endpoint: string,
  qs?: Record<string, string>,
  method?: string,
  body?: unknown,
) {
  const params = new URLSearchParams(qs || {}).toString();
  const url = `/plugin/${pluginId}/${endpoint}${params ? "?" + params : ""}`;
  const res = await fetch(url, {
    method: method || "GET",
    headers: body ? { "Content-Type": "application/json" } : undefined,
    body: body ? JSON.stringify(body) : undefined,
  });
  if (!res.ok) throw new Error(`${res.status} ${res.statusText}`);
  if (res.headers.get("content-type")?.includes("application/json")) {
    return res.json();
  }
  return res.text();
}

function fmtHours(h: number): string {
  if (h >= 1) return `${h.toFixed(1)}h`;
  const m = Math.round(h * 60);
  if (m >= 1) return `${m}m`;
  return "<1m";
}

function formatWakaTime(seconds: number): string {
  const mins = Math.round(seconds / 60);
  const h = Math.floor(mins / 60);
  const m = mins % 60;
  if (h > 0)
    return `${h} hr${h !== 1 ? "s" : ""} ${m} min${m !== 1 ? "s" : ""}`;
  return `${m} min${m !== 1 ? "s" : ""}`;
}

function BarChart({
  data,
}: {
  data: { name: string; value: number; maxValue: number }[];
}) {
  if (!data.length) return <p className="text-gray-500 text-xs">No data</p>;
  return (
    <div className="space-y-1.5">
      {data.slice(0, 10).map((d, i) => (
        <div key={d.name} className="flex items-center gap-2 text-xs">
          <span
            className="w-2.5 h-2.5 rounded-sm shrink-0"
            style={{ backgroundColor: COLORS[i % COLORS.length] }}
          />
          <span className="w-28 truncate text-gray-300" title={d.name}>
            {d.name || "(unknown)"}
          </span>
          <div className="flex-1 bg-gray-700 rounded h-3.5">
            <div
              className="h-3.5 rounded"
              style={{
                width: `${d.maxValue > 0 ? (d.value / d.maxValue) * 100 : 0}%`,
                backgroundColor: COLORS[i % COLORS.length],
                minWidth: d.value > 0 ? "2px" : "0",
              }}
            />
          </div>
          <span className="w-14 text-right text-gray-500 tabular-nums">
            {fmtHours(d.value / 3600)}
          </span>
        </div>
      ))}
    </div>
  );
}

function StatCard({ label, value }: { label: string; value: string }) {
  return (
    <div className="bg-gray-800 rounded-lg p-4 text-center">
      <div className="text-xl font-bold text-indigo-400">{value}</div>
      <div className="text-xs text-gray-500 mt-1">{label}</div>
    </div>
  );
}

function Panel({
  title,
  children,
}: {
  title: string;
  children: React.ReactNode;
}) {
  return (
    <div className="bg-gray-800/50 rounded-lg border border-gray-700 p-4">
      <h3 className="text-sm font-medium text-gray-400 mb-3">{title}</h3>
      {children}
    </div>
  );
}

function TimeseriesChart({ data }: { data: TimeseriesSeries[] }) {
  const w = 700;
  const h = 200;
  const pad = { top: 10, right: 20, bottom: 22, left: 50 };

  const allPoints = data.flatMap((s) => s.data.map((d) => d.seconds));
  const maxVal = Math.max(...allPoints, 1);
  const allTimes = data.flatMap((s) => s.data.map((d) => d.time));
  const minTime = Math.min(...allTimes);
  const maxTime = Math.max(...allTimes, minTime + 1);
  const timeRange = maxTime - minTime;

  const xScale = (t: number) =>
    pad.left + ((t - minTime) / timeRange) * (w - pad.left - pad.right);
  const yScale = (v: number) =>
    h - pad.bottom - (v / maxVal) * (h - pad.top - pad.bottom);

  return (
    <Panel title="Activity (hours per day)">
      <svg viewBox={`0 0 ${w} ${h}`} className="w-full">
        {[0, 0.25, 0.5, 0.75, 1].map((frac) => {
          const y = yScale(maxVal * frac);
          return (
            <g key={frac}>
              <line
                x1={pad.left}
                y1={y}
                x2={w - pad.right}
                y2={y}
                stroke="#374151"
                strokeDasharray="4 4"
              />
              <text
                x={pad.left - 8}
                y={y + 3}
                textAnchor="end"
                fill="#6b7280"
                fontSize="10"
              >
                {((maxVal * frac) / 3600).toFixed(1)}h
              </text>
            </g>
          );
        })}
        {data.map((series, si) => {
          if (!series.data.length) return null;
          const points = series.data
            .map((d) => `${xScale(d.time)},${yScale(d.seconds)}`)
            .join(" ");
          const color = COLORS[si % COLORS.length];
          const first = series.data[0];
          const last = series.data[series.data.length - 1];
          return (
            <g key={series.key}>
              <path
                d={`M${xScale(first.time)},${h - pad.bottom} ${series.data.map((d) => `L${xScale(d.time)},${yScale(d.seconds)}`).join(" ")} L${xScale(last.time)},${h - pad.bottom} Z`}
                fill={color}
                opacity="0.1"
              />
              <polyline
                points={points}
                fill="none"
                stroke={color}
                strokeWidth="1.5"
              />
            </g>
          );
        })}
        {data.slice(0, 8).map((s, i) => (
          <text
            key={s.key}
            x={pad.left + (i % 4) * 170}
            y={h + 5 + Math.floor(i / 4) * 14}
            fill="#9ca3af"
            fontSize="10"
          >
            <tspan fill={COLORS[i % COLORS.length]}>●</tspan> {s.key}
          </text>
        ))}
      </svg>
    </Panel>
  );
}

export default function CodingApp({ pluginId }: CodingAppProps) {
  const queryClient = useQueryClient();
  const [timeRange, setTimeRange] = useState("7d");
  const [heartbeatInput, setHeartbeatInput] = useState(
    JSON.stringify(
      {
        entity: "/src/main.rs",
        project: "panorama",
        language: "Rust",
        editor: "VSCode",
        operating_system: "Linux",
      },
      null,
      2,
    ),
  );
  const [filter, setFilter] = useState("");

  const statsQuery = useQuery({
    queryKey: ["wakatime-stats", timeRange, filter],
    queryFn: async () => {
      const qs: Record<string, string> = {};
      const parts = filter.split("=");
      if (parts.length === 2 && parts[0] && parts[1]) {
        qs[parts[0]] = parts[1];
      }
      return callPlugin(
        pluginId,
        `users/current/stats/${timeRange}`,
        qs,
      ) as Promise<WakaTimeStats>;
    },
  });

  const timeseriesQuery = useQuery({
    queryKey: ["timeseries", timeRange],
    queryFn: () =>
      callPlugin(pluginId, "stats", {
        range: timeRange,
        group_by: "project",
        aggregation: "timeseries",
        bucket: "day",
      }) as Promise<TimeseriesSeries[]>,
  });

  const sendMutation = useMutation({
    mutationFn: async () => {
      const body = JSON.parse(heartbeatInput);
      body.time = Date.now() / 1000;
      return callPlugin(pluginId, "heartbeat", {}, "POST", body);
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["wakatime-stats"] });
      queryClient.invalidateQueries({ queryKey: ["timeseries"] });
    },
  });

  const stats = statsQuery.data?.data;

  const buildBars = (entries: SummariesEntry[] | undefined) => {
    if (!entries?.length) return [];
    const maxVal = entries[0]?.total_seconds ?? 1;
    return entries.map((e) => ({
      name: e.name,
      value: e.total_seconds,
      maxValue: maxVal,
    }));
  };

  const isLoading = statsQuery.isLoading || timeseriesQuery.isLoading;
  const error = statsQuery.error || timeseriesQuery.error;

  if (isLoading) {
    return (
      <div className="max-w-6xl mx-auto p-6 text-gray-400">Loading...</div>
    );
  }

  return (
    <div className="max-w-6xl mx-auto p-6 space-y-5">
      {/* Header */}
      <div className="flex items-center justify-between flex-wrap gap-3">
        <h2 className="text-lg font-semibold text-white">Coding Activity</h2>
        <div className="flex gap-1.5">
          {["24h", "7d", "30d", "90d", "365d", "all"].map((r) => (
            <button
              key={r}
              onClick={() => setTimeRange(r)}
              className={`px-3 py-1 rounded text-xs font-medium ${
                timeRange === r
                  ? "bg-indigo-600 text-white"
                  : "bg-gray-700 text-gray-300 hover:bg-gray-600"
              }`}
            >
              {r}
            </button>
          ))}
        </div>
      </div>

      {/* Errors */}
      {error && (
        <div className="bg-red-900/50 border border-red-700 rounded p-3 text-red-300 text-xs">
          {String(error)}
        </div>
      )}
      {sendMutation.error && (
        <div className="bg-red-900/50 border border-red-700 rounded p-3 text-red-300 text-xs">
          {String(sendMutation.error)}
        </div>
      )}

      {/* Stat cards */}
      {stats && (
        <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
          <StatCard
            label="Total"
            value={
              stats.human_readable_total || formatWakaTime(stats.total_seconds)
            }
          />
          <StatCard
            label="Daily Avg"
            value={
              stats.human_readable_daily_average ||
              formatWakaTime(stats.daily_average)
            }
          />
          {stats.best_day && (
            <StatCard
              label={`Best Day (${stats.best_day.date})`}
              value={stats.best_day.text}
            />
          )}
          <StatCard
            label="Languages"
            value={String(stats.languages?.length || 0)}
          />
        </div>
      )}

      {/* Filter */}
      <div className="flex gap-2 items-center">
        <span className="text-xs text-gray-500">Filter:</span>
        <input
          type="text"
          placeholder="e.g. project=panorama"
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
          className="bg-gray-800 text-gray-200 rounded px-2.5 py-1 text-xs border border-gray-700 w-44 focus:outline-none focus:border-indigo-500"
        />
        {filter && (
          <button
            onClick={() => setFilter("")}
            className="text-gray-500 hover:text-gray-300 text-xs"
          >
            clear
          </button>
        )}
      </div>

      {/* Test heartbeat */}
      <div className="bg-gray-800 rounded-lg border border-gray-700 p-4 space-y-3">
        <h3 className="text-xs font-medium text-gray-400">
          Send Test Heartbeat
        </h3>
        <textarea
          className="w-full h-28 bg-gray-900 text-green-400 font-mono text-xs p-3 rounded border border-gray-700 resize-y"
          value={heartbeatInput}
          onChange={(e) => setHeartbeatInput(e.target.value)}
        />
        <button
          onClick={() => sendMutation.mutate()}
          disabled={sendMutation.isPending}
          className="px-4 py-2 bg-indigo-600 text-white rounded text-xs font-medium hover:bg-indigo-500 disabled:opacity-50"
        >
          {sendMutation.isPending ? "Sending..." : "Send Heartbeat"}
        </button>
      </div>

      {/* Leaderboards */}
      <div className="grid grid-cols-1 md:grid-cols-2 gap-5">
        <Panel title="Per Project">
          <BarChart data={buildBars(stats?.projects)} />
        </Panel>
        <Panel title="Languages">
          <BarChart data={buildBars(stats?.languages)} />
        </Panel>
        <Panel title="Editors">
          <BarChart data={buildBars(stats?.editors)} />
        </Panel>
        <Panel title="Operating Systems">
          <BarChart data={buildBars(stats?.operating_systems)} />
        </Panel>
        <Panel title="Machines">
          <BarChart data={buildBars(stats?.machines)} />
        </Panel>
        <Panel title="Categories">
          <BarChart data={buildBars(stats?.categories)} />
        </Panel>
      </div>

      {/* Timeseries */}
      {timeseriesQuery.data && timeseriesQuery.data.length > 0 && (
        <TimeseriesChart data={timeseriesQuery.data} />
      )}

      {/* Activity heatmap */}
      <Panel title="Activity (Past Year)">
        <img
          src={`/plugin/${pluginId}/activity/chart/current.svg`}
          alt="Activity chart"
          className="w-full max-w-4xl"
        />
      </Panel>
    </div>
  );
}
