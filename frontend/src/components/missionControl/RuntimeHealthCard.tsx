import { NeuralPanel } from "./NeuralPanel";
import type { RuntimeHealth } from "../../types";

export type RuntimeHealthCardProps = {
  health?: RuntimeHealth | null;
  rttMs?: number | null;
};

type HealthRow = {
  label: string;
  value: string;
  meter: number | null;
};

function num(value: unknown): number | null {
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function clampMeter(value: number | null): number {
  if (value == null) return 0;
  return Math.max(0, Math.min(100, value));
}

function formatPercent(value: number | null): string {
  return value == null ? "-" : `${Math.round(value)}%`;
}

function formatRate(bytesPerSec: number | null): string {
  if (bytesPerSec == null) return "-";
  if (bytesPerSec >= 1024 * 1024) return `${(bytesPerSec / 1024 / 1024).toFixed(1)} MB/s`;
  if (bytesPerSec >= 1024) return `${Math.round(bytesPerSec / 1024)} KB/s`;
  return `${Math.round(bytesPerSec)} B/s`;
}

function formatTemp(value: number | null): string {
  return value == null ? "-" : `${Math.round(value)} C`;
}

function diskMeter(bytesPerSec: number | null): number | null {
  if (bytesPerSec == null) return null;
  const baseline = 128 * 1024 * 1024;
  return Math.min(100, (bytesPerSec / baseline) * 100);
}

export function RuntimeHealthCard({ health, rttMs }: RuntimeHealthCardProps) {
  const cpu = num(health?.cpu_percent);
  const ram = num(health?.ram_percent ?? health?.memory_pressure_percent);
  const readRate = num(health?.disk_read_bytes_per_sec);
  const writeRate = num(health?.disk_write_bytes_per_sec);
  const diskRate = readRate == null && writeRate == null ? null : (readRate ?? 0) + (writeRate ?? 0);
  const temp = num(health?.temperature_celsius);
  const load = num(health?.load_average_1m);

  const rows: HealthRow[] = [
    {
      label: "CPU",
      value: cpu == null && load != null ? `Load ${load.toFixed(2)}` : formatPercent(cpu),
      meter: cpu,
    },
    { label: "RAM", value: formatPercent(ram), meter: ram },
    { label: "Disk I/O", value: formatRate(diskRate), meter: diskMeter(diskRate) },
    { label: "Temperature", value: formatTemp(temp), meter: temp == null ? null : Math.min(100, temp) },
    { label: "Latency", value: rttMs == null ? "-" : `${Math.round(rttMs)} ms`, meter: null },
  ];

  return (
    <NeuralPanel title="Runtime Health" tag={health ? "LIVE" : "WAITING"} tagTone={health ? "cyan" : "warn"} className="nw-card--runtime-health">
      <div className="nw-health-list">
        {rows.map((row) => (
          <div className="nw-health-row" key={row.label}>
            <div className="nw-health-k">{row.label}</div>
            <div className="nw-health-v">{row.value}</div>
            <div className="nw-meter" aria-hidden="true">
              <div className="nw-meter-fill" style={{ width: `${clampMeter(row.meter)}%` }} />
            </div>
          </div>
        ))}
      </div>
    </NeuralPanel>
  );
}
