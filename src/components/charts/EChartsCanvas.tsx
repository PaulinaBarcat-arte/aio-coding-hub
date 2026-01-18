// Usage:
// - Generic ECharts wrapper used by Home "花费" charts and other dashboard-like views.
// - Provides lazy ECharts import, ResizeObserver auto-resize, and option updates.

import { useEffect, useRef } from "react";
import type { ECharts, EChartsOption } from "echarts";
import { cn } from "../../utils/cn";

export type EChartsCanvasProps = {
  option: EChartsOption;
  className?: string;
};

export function EChartsCanvas({ option, className }: EChartsCanvasProps) {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const chartRef = useRef<ECharts | null>(null);
  const optionRef = useRef<EChartsOption>(option);

  useEffect(() => {
    optionRef.current = option;
    if (chartRef.current) {
      chartRef.current.setOption(option, { notMerge: true, lazyUpdate: true });
    }
  }, [option]);

  useEffect(() => {
    let disposed = false;
    let observer: ResizeObserver | null = null;
    let chart: ECharts | null = null;

    const init = async () => {
      const el = containerRef.current;
      if (!el) return;

      const echarts = await import("echarts");
      if (disposed) return;

      chart = echarts.init(el, undefined, { renderer: "canvas" });
      chartRef.current = chart;
      chart.setOption(optionRef.current, { notMerge: true, lazyUpdate: true });

      observer = new ResizeObserver(() => {
        chart?.resize();
      });
      observer.observe(el);
    };

    void init().catch(() => {});

    return () => {
      disposed = true;
      observer?.disconnect();
      chartRef.current = null;
      chart?.dispose();
    };
  }, []);

  return <div ref={containerRef} className={cn("h-full w-full", className)} />;
}
