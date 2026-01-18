import { useEffect, useMemo, useRef } from "react";
import type { ECharts, EChartsOption } from "echarts";
import type { UsageHourlyRow } from "../services/usage";
import { cn } from "../utils/cn";
import { buildRecentDayKeys } from "../utils/dateKeys";

function formatTokensMillions(value: number) {
  if (!Number.isFinite(value) || value === 0) return "0";
  const millions = value / 1_000_000;
  if (millions >= 1) {
    return `${millions.toFixed(1)}M`;
  }
  if (value >= 1000) {
    return `${(value / 1000).toFixed(1)}K`;
  }
  return String(Math.round(value));
}

/**
 * 计算"漂亮"的 Y 轴上限，使刻度间隔固定且易读
 * 返回 { max, interval } 以确保 Y 轴刻度均匀分布
 */
function computeNiceYAxis(maxValue: number, tickCount = 5): { max: number; interval: number } {
  if (maxValue <= 0) {
    return { max: 1_000_000, interval: 200_000 };
  }

  // 计算粗略的间隔
  const roughInterval = maxValue / tickCount;

  // 计算数量级
  const magnitude = Math.pow(10, Math.floor(Math.log10(roughInterval)));

  // 选择一个"漂亮"的间隔倍数：1, 2, 2.5, 5, 10
  const normalized = roughInterval / magnitude;
  let niceMultiplier: number;
  if (normalized <= 1) {
    niceMultiplier = 1;
  } else if (normalized <= 2) {
    niceMultiplier = 2;
  } else if (normalized <= 2.5) {
    niceMultiplier = 2.5;
  } else if (normalized <= 5) {
    niceMultiplier = 5;
  } else {
    niceMultiplier = 10;
  }

  const niceInterval = niceMultiplier * magnitude;
  const niceMax = Math.ceil(maxValue / niceInterval) * niceInterval;

  return { max: niceMax, interval: niceInterval };
}

function toDateLabel(dayKey: string) {
  const mmdd = dayKey.slice(5);
  return mmdd.replace("-", "/");
}

export function UsageTokensChart({
  rows,
  days = 15,
  className,
}: {
  rows: UsageHourlyRow[];
  days?: number;
  className?: string;
}) {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const chartRef = useRef<ECharts | null>(null);
  const optionRef = useRef<EChartsOption | null>(null);

  const dayKeys = useMemo(() => buildRecentDayKeys(days), [days]);

  const tokensByDay = useMemo(() => {
    const map = new Map<string, number>();
    for (const row of rows) {
      const day = row.day;
      if (!day) continue;
      const prev = map.get(day) ?? 0;
      const next = prev + (Number(row.total_tokens) || 0);
      map.set(day, next);
    }
    return map;
  }, [rows]);

  const seriesData = useMemo(() => {
    return dayKeys.map((day) => tokensByDay.get(day) ?? 0);
  }, [dayKeys, tokensByDay]);

  const option = useMemo(() => {
    const xLabels = dayKeys.map(toDateLabel);
    const maxY = Math.max(0, ...seriesData);
    const { max: yMax, interval: yInterval } = computeNiceYAxis(maxY, 5);
    const lineColor = "#0052FF";
    const gridLine = "rgba(0,82,255,0.15)";

    const opt: EChartsOption = {
      animation: false,
      grid: { left: 0, right: 16, top: 8, bottom: 24, containLabel: true },
      tooltip: {
        trigger: "axis",
        axisPointer: { type: "line" },
        valueFormatter: (value) => formatTokensMillions(Number(value)),
      },
      xAxis: {
        type: "category",
        data: xLabels,
        boundaryGap: false,
        axisLabel: { color: "#64748b", fontSize: 10, interval: 2 },
        axisLine: { lineStyle: { color: "rgba(15,23,42,0.12)" } },
        axisTick: { show: false },
      },
      yAxis: {
        type: "value",
        min: 0,
        max: yMax,
        interval: yInterval,
        axisLabel: {
          color: "#64748b",
          fontSize: 10,
          formatter: (value: number) => formatTokensMillions(value),
        },
        axisTick: { show: false },
        axisLine: { show: false },
        splitLine: { lineStyle: { color: gridLine, type: "dashed" } },
      },
      series: [
        {
          name: "total_tokens",
          type: "line",
          data: seriesData,
          showSymbol: false,
          smooth: true,
          lineStyle: { color: lineColor, width: 3 },
          areaStyle: {
            color: {
              type: "linear",
              x: 0,
              y: 0,
              x2: 0,
              y2: 1,
              colorStops: [
                { offset: 0, color: "rgba(0,82,255,0.25)" },
                { offset: 1, color: "rgba(0,82,255,0.0)" },
              ],
            },
          },
          emphasis: { focus: "series" },
        },
      ],
    };

    return opt;
  }, [dayKeys, seriesData]);

  useEffect(() => {
    optionRef.current = option;
    const chart = chartRef.current;
    if (chart) {
      chart.setOption(option, { notMerge: true, lazyUpdate: true });
    }
  }, [option]);

  useEffect(() => {
    let disposed = false;
    let observer: ResizeObserver | null = null;
    let chart: ECharts | null = null;

    const load = async () => {
      const el = containerRef.current;
      if (!el) return;

      const echarts = await import("echarts");
      if (disposed) return;

      chart = echarts.init(el, undefined, { renderer: "canvas" });
      chartRef.current = chart;
      if (optionRef.current) {
        chart.setOption(optionRef.current, { notMerge: true, lazyUpdate: true });
      }

      observer = new ResizeObserver(() => {
        chart?.resize();
      });
      observer.observe(el);
    };

    void load().catch(() => {});

    return () => {
      disposed = true;
      observer?.disconnect();
      chartRef.current = null;
      chart?.dispose();
    };
  }, []);

  return <div ref={containerRef} className={cn("h-full w-full", className)} />;
}
