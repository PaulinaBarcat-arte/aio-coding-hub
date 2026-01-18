import type { HTMLAttributes } from "react";
import { cn } from "../utils/cn";

export type CardPadding = "none" | "sm" | "md";

export type CardProps = HTMLAttributes<HTMLDivElement> & {
  padding?: CardPadding;
};

const PADDING_CLASS: Record<CardPadding, string> = {
  none: "",
  sm: "p-4",
  md: "p-6",
};

export function Card({ padding = "md", className, ...props }: CardProps) {
  return (
    <div
      className={cn(
        "overflow-hidden rounded-2xl border border-slate-200 bg-white shadow-card",
        PADDING_CLASS[padding],
        className
      )}
      {...props}
    />
  );
}
