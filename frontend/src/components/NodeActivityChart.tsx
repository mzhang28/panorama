import { ResponsiveBar } from "@nivo/bar";
import { useMemo } from "react";
import type { Node } from "../api/client";

interface NodeActivityChartProps {
  nodes: Node[];
}

interface Bucket {
  label: string;
  count: number;
  [key: string]: string | number;
}

function bucketNodes(nodes: Node[]): Bucket[] {
  if (nodes.length === 0) return [];

  const now = Date.now();
  const timestamps = nodes.map((n) => new Date(n.created_at).getTime());
  const oldest = Math.min(...timestamps);
  const rangeMs = now - oldest;

  // Use hourly buckets if range < 24h, otherwise daily
  const useHourly = rangeMs < 24 * 60 * 60 * 1000;
  const bucketMs = useHourly ? 60 * 60 * 1000 : 24 * 60 * 60 * 1000;

  // Determine number of buckets (aim for ~12-24 buckets)
  const numBuckets = Math.min(Math.max(Math.ceil(rangeMs / bucketMs), 6), 24);
  const adjustedBucketMs = rangeMs / numBuckets;

  const buckets: Bucket[] = [];
  for (let i = 0; i < numBuckets; i++) {
    const bucketStart = now - (numBuckets - i) * adjustedBucketMs;
    const bucketEnd = now - (numBuckets - i - 1) * adjustedBucketMs;

    const count = nodes.filter((n) => {
      const t = new Date(n.created_at).getTime();
      return t >= bucketStart && t < bucketEnd;
    }).length;

    const label = useHourly
      ? new Date(bucketStart).toLocaleTimeString(undefined, {
          hour: "numeric",
          minute: "2-digit",
        })
      : new Date(bucketStart).toLocaleDateString(undefined, {
          month: "short",
          day: "numeric",
        });

    buckets.push({ label, count });
  }

  return buckets;
}

export function NodeActivityChart({ nodes }: NodeActivityChartProps) {
  const data = useMemo(() => bucketNodes(nodes), [nodes]);

  if (data.length === 0) {
    return (
      <div className="bg-[var(--surface-elevated)] border border-[var(--border)] rounded-[var(--radius-md)] p-[var(--space-6)]">
        <h3 className="text-[var(--text-muted)] text-xs font-medium uppercase tracking-wide mb-[var(--space-4)]">
          Activity Timeline
        </h3>
        <p className="text-[var(--text-dim)] text-sm text-center py-8">
          No activity data available
        </p>
      </div>
    );
  }

  return (
    <div className="bg-[var(--surface-elevated)] border border-[var(--border)] rounded-[var(--radius-md)] p-[var(--space-6)]">
      <h3 className="text-[var(--text-muted)] text-xs font-medium uppercase tracking-wide mb-[var(--space-2)]">
        Activity Timeline
      </h3>
      <div style={{ height: 200 }}>
        <ResponsiveBar
          data={data}
          keys={["count"]}
          indexBy="label"
          margin={{ top: 8, right: 8, bottom: 40, left: 40 }}
          padding={0.3}
          valueScale={{ type: "linear" }}
          colors={["var(--accent)"]}
          borderRadius={4}
          axisBottom={{
            tickSize: 5,
            tickPadding: 5,
            tickRotation: -45,
            tickValues: Math.min(data.length, 12),
          }}
          axisLeft={{
            tickSize: 5,
            tickPadding: 5,
            tickValues: 5,
          }}
          gridYValues={5}
          theme={{
            text: {
              fontSize: 11,
              fill: "var(--text-muted)",
            },
            grid: {
              line: {
                stroke: "var(--border)",
                strokeWidth: 1,
              },
            },
            axis: {
              domain: {
                line: {
                  stroke: "var(--border)",
                  strokeWidth: 1,
                },
              },
              ticks: {
                line: {
                  stroke: "var(--border)",
                  strokeWidth: 1,
                },
              },
            },
          }}
          enableLabel={false}
          animate={true}
          motionConfig="gentle"
        />
      </div>
    </div>
  );
}
