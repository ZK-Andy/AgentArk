import { Box, Typography } from "@mui/material";
import { useMemo } from "react";
import ReactECharts from "echarts-for-react";

const AGENTARK_CHART_LANGUAGE = "agentark-chart";
const MAX_CHART_ROWS = 160;
const MAX_CHART_SERIES = 8;
const AXIS_LABEL_COLOR = "#8fb2d1";
const AXIS_LINE_COLOR = "rgba(108, 156, 212, 0.22)";
const SPLIT_LINE_COLOR = "rgba(108, 156, 212, 0.16)";
const TOOLTIP_BG = "rgba(12, 18, 28, 0.96)";
const TOOLTIP_BORDER = "rgba(108, 156, 212, 0.28)";
const LINE_COLORS = ["#00f5a3", "#1ecbff", "#f6bd36", "#a56eff", "#d8ad78", "#ff8f8f"];
const PIE_COLORS = ["#d8ad78", "#50d6a6", "#7cc7ff", "#f3c75f", "#a56eff", "#ff8f8f"];

type ChartRecord = Record<string, unknown>;
type ChartKind = "bar" | "line" | "area" | "scatter" | "pie" | "doughnut";

type ChartSeries = {
  key: string;
  name: string;
  kind?: ChartKind;
};

type ChartModel =
  | {
      ok: true;
      title: string;
      subtitle: string;
      option: ChartRecord;
      height: number;
    }
  | {
      ok: false;
      message: string;
    };

function isRecord(value: unknown): value is ChartRecord {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function textValue(value: unknown, fallback = ""): string {
  if (typeof value === "string" && value.trim()) return value.trim();
  if (typeof value === "number" || typeof value === "boolean")
    return String(value);
  return fallback;
}

function numberValue(value: unknown): number | null {
  if (typeof value === "number" && Number.isFinite(value)) return value;
  if (typeof value !== "string") return null;
  const parsed = Number(value.replace(/[$,%\s,]/g, ""));
  return Number.isFinite(parsed) ? parsed : null;
}

function clampNumber(
  value: unknown,
  fallback: number,
  min: number,
  max: number,
): number {
  const parsed = numberValue(value);
  if (parsed == null) return fallback;
  return Math.min(max, Math.max(min, parsed));
}

function explicitChartKind(value: unknown): ChartKind | null {
  const normalized = textValue(value).toLowerCase();
  switch (normalized) {
    case "bar":
    case "line":
    case "area":
    case "scatter":
    case "pie":
    case "doughnut":
      return normalized;
    default:
      return null;
  }
}

function chartKind(value: unknown): ChartKind {
  return explicitChartKind(value) || "bar";
}

function chartColor(index: number, palette = LINE_COLORS): string {
  return palette[index % palette.length] || palette[0];
}

function hexToRgba(hex: string, alpha: number): string {
  const normalized = hex.replace("#", "");
  if (!/^[0-9a-f]{6}$/i.test(normalized)) return `rgba(216, 173, 120, ${alpha})`;
  const r = Number.parseInt(normalized.slice(0, 2), 16);
  const g = Number.parseInt(normalized.slice(2, 4), 16);
  const b = Number.parseInt(normalized.slice(4, 6), 16);
  return `rgba(${r}, ${g}, ${b}, ${alpha})`;
}

function goldBarGradient(): ChartRecord {
  return {
    type: "linear",
    x: 0,
    y: 0,
    x2: 0,
    y2: 1,
    colorStops: [
      { offset: 0, color: "#f1d6ad" },
      { offset: 0.36, color: "#d8ad78" },
      { offset: 1, color: "#8d6841" },
    ],
  };
}

function firstDataKey(
  rows: ChartRecord[],
  predicate: (value: unknown) => boolean,
): string {
  const keys = new Set<string>();
  for (const row of rows) {
    Object.keys(row).forEach((key) => keys.add(key));
  }
  for (const key of keys) {
    if (rows.some((row) => predicate(row[key]))) return key;
  }
  return keys.values().next().value || "";
}

function inferCategoryKey(rows: ChartRecord[], preferred: unknown): string {
  const explicit = textValue(preferred);
  if (explicit) return explicit;
  return firstDataKey(rows, (value) => numberValue(value) == null);
}

function looksTemporalCategoryValue(value: unknown): boolean {
  const text = textValue(value);
  if (!text) return false;
  if (Number.isFinite(Date.parse(text))) return true;
  return /\d{1,2}:\d{2}/.test(text) || /\d{4}[-/]\d{1,2}[-/]\d{1,2}/.test(text);
}

function inferChartKind(spec: ChartRecord, rows: ChartRecord[]): ChartKind {
  const explicit = explicitChartKind(spec.type);
  if (explicit) return explicit;
  const categoryKey = inferCategoryKey(rows, spec.x);
  const temporalCount = rows.filter((row) =>
    looksTemporalCategoryValue(row[categoryKey]),
  ).length;
  return temporalCount >= Math.min(2, rows.length) ? "line" : "bar";
}

function inferNumericKeys(rows: ChartRecord[], categoryKey: string): string[] {
  const keys = new Set<string>();
  for (const row of rows) {
    Object.keys(row).forEach((key) => {
      if (key !== categoryKey) keys.add(key);
    });
  }
  return Array.from(keys)
    .filter((key) => rows.some((row) => numberValue(row[key]) != null))
    .slice(0, MAX_CHART_SERIES);
}

function seriesFromSpec(
  spec: ChartRecord,
  rows: ChartRecord[],
  categoryKey: string,
): ChartSeries[] {
  const rawSeries = spec.series;
  if (Array.isArray(rawSeries)) {
    const series = rawSeries
      .map((item): ChartSeries | null => {
        if (typeof item === "string") {
          const key = item.trim();
          return key ? { key, name: key } : null;
        }
        if (!isRecord(item)) return null;
        const key = textValue(item.key);
        if (!key) return null;
        const explicitKind = explicitChartKind(item.type);
        return {
          key,
          name: textValue(item.name, textValue(item.label, key)),
          kind: explicitKind || undefined,
        };
      })
      .filter((item): item is ChartSeries => item !== null);
    if (series.length > 0) return series.slice(0, MAX_CHART_SERIES);
  }

  return inferNumericKeys(rows, categoryKey).map((key) => ({ key, name: key }));
}

function valueForRow(row: ChartRecord, key: string): number | null {
  return numberValue(row[key]);
}

function buildDataZoom(rows: ChartRecord[]): ChartRecord[] | undefined {
  if (rows.length <= 16) return undefined;
  return [
    { type: "inside", xAxisIndex: 0, throttle: 80 },
    {
      type: "slider",
      xAxisIndex: 0,
      height: 18,
      bottom: 6,
      borderColor: "rgba(108, 156, 212, 0.2)",
      fillerColor: "rgba(47, 212, 255, 0.14)",
      handleStyle: { color: AXIS_LABEL_COLOR },
      textStyle: { color: AXIS_LABEL_COLOR },
      dataBackground: {
        lineStyle: { color: "rgba(47, 212, 255, 0.34)" },
        areaStyle: { color: "rgba(47, 212, 255, 0.10)" },
      },
      selectedDataBackground: {
        lineStyle: { color: "rgba(47, 212, 255, 0.56)" },
        areaStyle: { color: "rgba(47, 212, 255, 0.18)" },
      },
    },
  ];
}

function buildAxisOption(spec: ChartRecord, rows: ChartRecord[], kind: ChartKind): ChartRecord {
  const categoryKey = inferCategoryKey(rows, spec.x);
  const categories = rows.map((row, index) =>
    textValue(row[categoryKey], String(index + 1)),
  );
  const series = seriesFromSpec(spec, rows, categoryKey);
  const dataZoom = buildDataZoom(rows);
  const chartSeries = series.map((entry, index) => {
    const seriesKind =
      entry.kind && entry.kind !== "pie" && entry.kind !== "doughnut"
        ? entry.kind
        : kind;
    const lineColor = chartColor(index);
    if (seriesKind === "bar") {
      return {
        name: entry.name,
        type: "bar",
        barWidth: "42%",
        barMaxWidth: 34,
        barMinWidth: 3,
        itemStyle: {
          borderRadius: [5, 5, 1, 1],
          color: index === 0 ? goldBarGradient() : lineColor,
        },
        emphasis: {
          itemStyle: {
            color: index === 0 ? "#f4ddb9" : lineColor,
          },
        },
        data: rows.map((row) => valueForRow(row, entry.key)),
      };
    }
    return {
      name: entry.name,
      type: seriesKind === "area" ? "line" : seriesKind,
      smooth: seriesKind === "line" || seriesKind === "area",
      symbol: "circle",
      symbolSize: seriesKind === "scatter" ? 8 : 6,
      showSymbol: rows.length <= 32,
      itemStyle: {
        color: lineColor,
        borderColor: "#f7fbff",
        borderWidth: 1.5,
      },
      lineStyle:
        seriesKind === "scatter"
          ? undefined
          : {
              width: 2.2,
              type: index >= 2 ? "dashed" : "solid",
              color: lineColor,
            },
      areaStyle:
        seriesKind === "area"
          ? {
              color: {
                type: "linear",
                x: 0,
                y: 0,
                x2: 0,
                y2: 1,
                colorStops: [
                  { offset: 0, color: hexToRgba(lineColor, 0.30) },
                  { offset: 1, color: hexToRgba(lineColor, 0.04) },
                ],
              },
            }
          : undefined,
      data: rows.map((row) => valueForRow(row, entry.key)),
    };
  });
  const hasMultipleSeries = chartSeries.length > 1;

  return {
    backgroundColor: "transparent",
    animationDuration: 450,
    color: LINE_COLORS,
    tooltip: {
      trigger: "axis",
      confine: true,
      backgroundColor: TOOLTIP_BG,
      borderColor: TOOLTIP_BORDER,
      textStyle: { color: "#d8edff" },
      axisPointer: {
        type: kind === "bar" ? "shadow" : "line",
        lineStyle: { color: "rgba(143, 178, 209, 0.52)" },
        shadowStyle: { color: "rgba(47, 212, 255, 0.06)" },
      },
    },
    legend: {
      top: 4,
      left: hasMultipleSeries ? "center" : undefined,
      right: hasMultipleSeries ? undefined : 8,
      icon: "circle",
      itemWidth: 11,
      itemHeight: 11,
      textStyle: { color: "#bdd7ee" },
    },
    grid: {
      left: 18,
      right: 12,
      top: 42,
      bottom: dataZoom ? 52 : 28,
      containLabel: true,
    },
    xAxis: {
      type: "category",
      data: categories,
      axisLabel: {
        color: AXIS_LABEL_COLOR,
        fontSize: 10,
        hideOverlap: true,
        rotate: rows.length > 10 ? 25 : 0,
      },
      axisLine: { lineStyle: { color: AXIS_LINE_COLOR } },
      axisTick: { show: false },
    },
    yAxis: {
      type: "value",
      axisLabel: { color: AXIS_LABEL_COLOR },
      splitLine: { lineStyle: { color: SPLIT_LINE_COLOR } },
    },
    dataZoom,
    series: chartSeries,
  };
}

function buildPieOption(spec: ChartRecord, rows: ChartRecord[], kind: ChartKind): ChartRecord {
  const categoryKey = inferCategoryKey(rows, spec.x);
  const series = seriesFromSpec(spec, rows, categoryKey);
  const valueKey = series[0]?.key;
  const data = valueKey
    ? rows
        .map((row, index) => ({
          name: textValue(row[categoryKey], String(index + 1)),
          value: valueForRow(row, valueKey),
        }))
        .filter((row) => row.value != null)
    : [];

  return {
    backgroundColor: "transparent",
    animationDuration: 450,
    color: PIE_COLORS,
    tooltip: {
      trigger: "item",
      confine: true,
      backgroundColor: TOOLTIP_BG,
      borderColor: TOOLTIP_BORDER,
      textStyle: { color: "#d8edff" },
    },
    legend: {
      orient: "vertical",
      right: 8,
      top: 18,
      bottom: 18,
      textStyle: { color: "#bdd7ee" },
    },
    series: [
      {
        name: series[0]?.name || textValue(spec.title, "Value"),
        type: "pie",
        radius: kind === "doughnut" ? ["44%", "68%"] : "68%",
        center: ["40%", "52%"],
        avoidLabelOverlap: true,
        label: { color: "rgba(255, 248, 237, 0.8)" },
        labelLine: { lineStyle: { color: "rgba(255, 248, 237, 0.3)" } },
        data,
      },
    ],
  };
}

function buildChartModel(code: string): ChartModel {
  let parsed: unknown;
  try {
    parsed = JSON.parse(code);
  } catch {
    return { ok: false, message: "Chart block is not valid JSON." };
  }
  if (!isRecord(parsed)) {
    return { ok: false, message: "Chart block must be a JSON object." };
  }
  const rows = Array.isArray(parsed.data)
    ? parsed.data.filter(isRecord).slice(0, MAX_CHART_ROWS)
    : [];
  if (rows.length === 0) {
    return { ok: false, message: "Chart block does not include tabular data." };
  }
  const kind = inferChartKind(parsed, rows);
  const option =
    kind === "pie" || kind === "doughnut"
      ? buildPieOption(parsed, rows, kind)
      : buildAxisOption(parsed, rows, kind);
  return {
    ok: true,
    title: textValue(parsed.title, "Chart"),
    subtitle: textValue(parsed.subtitle),
    option,
    height: clampNumber(parsed.height, 310, 220, 520),
  };
}

export function markdownFenceLanguage(className = ""): string {
  const token = className
    .split(/\s+/)
    .map((part) => part.trim())
    .find((part) => part.length > 0);
  return (token || "").replace(/^language-/, "").toLowerCase();
}

export function isAgentArkChartFence(className = ""): boolean {
  return markdownFenceLanguage(className) === AGENTARK_CHART_LANGUAGE;
}

export function InlineAgentArkChart({ code }: { code: string }) {
  const model = useMemo(() => buildChartModel(code), [code]);

  if (!model.ok) {
    return (
      <Box className="chat-inline-chart chat-inline-chart-error">
        <Typography className="chat-inline-chart-title" variant="body2">
          Chart unavailable
        </Typography>
        <Typography className="chat-inline-chart-subtitle" variant="caption">
          {model.message}
        </Typography>
      </Box>
    );
  }

  return (
    <Box className="chat-inline-chart">
      <Box className="chat-inline-chart-header">
        <Typography className="chat-inline-chart-title" variant="body2">
          {model.title}
        </Typography>
        {model.subtitle ? (
          <Typography className="chat-inline-chart-subtitle" variant="caption">
            {model.subtitle}
          </Typography>
        ) : null}
      </Box>
      <ReactECharts
        option={model.option}
        notMerge
        lazyUpdate
        opts={{ renderer: "svg" }}
        style={{ width: "100%", height: model.height }}
      />
    </Box>
  );
}
