import { Box, Stack, Typography } from "@mui/material";
import ReactECharts from "echarts-for-react";

type MetricLegendRow = {
  label: string;
  value: string;
};

type Props = {
  title: string;
  value: string;
  values: number[];
  rows: MetricLegendRow[];
  palette: string[];
};

export function MetricBarCard({ title, value, values, rows, palette }: Props) {
  const option = {
    backgroundColor: "transparent",
    animationDuration: 320,
    grid: { left: 0, right: 0, top: 8, bottom: 2, containLabel: false },
    tooltip: {
      trigger: "axis",
      backgroundColor: "rgba(6,14,28,0.95)",
      borderColor: "rgba(84,198,255,0.22)",
      textStyle: { color: "#d8edff" },
      axisPointer: {
        type: "shadow",
        shadowStyle: {
          color: "rgba(84,198,255,0.06)",
        },
      },
    },
    xAxis: {
      type: "category",
      data: rows.map((row) => row.label),
      boundaryGap: true,
      axisLine: { show: false },
      axisTick: { show: false },
      axisLabel: { show: false },
    },
    yAxis: {
      type: "value",
      max: (axis: { max: number }) => (axis.max > 0 ? axis.max * 1.16 : 1),
      splitLine: { show: false },
      axisLine: { show: false },
      axisTick: { show: false },
      axisLabel: { show: false },
    },
    series: [
      {
        type: "bar",
        data: values.map((entry, index) => ({
          value: entry,
          itemStyle: {
            color: palette[index % palette.length],
            borderRadius: [999, 999, 999, 999],
            shadowBlur: 8,
            shadowColor: "rgba(0,0,0,0.18)",
          },
        })),
        showBackground: true,
        backgroundStyle: {
          color: "rgba(108,156,212,0.05)",
          borderRadius: [999, 999, 999, 999],
        },
        barWidth: 8,
        barMaxWidth: 8,
        barMinHeight: 4,
        barCategoryGap: "78%",
      },
    ],
  };

  return (
    <Box
      className="list-shell"
      sx={{
        p: 1.6,
        borderRadius: "12px",
        border: "1px solid rgba(108,156,212,0.18)",
        background: "linear-gradient(170deg, rgba(6,15,29,0.95), rgba(3,9,21,0.9))",
      }}
    >
      <Typography variant="subtitle1" sx={{ color: "#d8edff", fontWeight: 600 }}>
        {title}
      </Typography>
      <Typography variant="h4" sx={{ color: "#f3fbff", fontWeight: 700, mb: 0.4 }}>
        {value}
      </Typography>
      <ReactECharts option={option} style={{ height: 84 }} />
      <Stack spacing={0.5} sx={{ mt: 0.8 }}>
        {rows.map((row, index) => (
          <Stack
            key={`${title}-${row.label}-${index}`}
            direction="row"
            justifyContent="space-between"
            alignItems="center"
          >
            <Stack direction="row" spacing={0.8} alignItems="center" sx={{ minWidth: 0 }}>
              <Box
                sx={{
                  width: 8,
                  height: 8,
                  borderRadius: "50%",
                  bgcolor: palette[index % palette.length],
                  flex: "0 0 auto",
                }}
              />
              <Typography variant="body2" noWrap title={row.label}>
                {row.label}
              </Typography>
            </Stack>
            <Typography variant="body2" color="text.secondary">
              {row.value}
            </Typography>
          </Stack>
        ))}
      </Stack>
    </Box>
  );
}
