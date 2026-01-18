import type { ReactNode } from "react";
import { cn } from "../utils/cn";

export type SettingsRowProps = {
  label: string;
  children: ReactNode;
  className?: string;
};

export function SettingsRow({ label, children, className }: SettingsRowProps) {
  return (
    <div
      className={cn(
        "flex flex-col gap-2 py-3 sm:flex-row sm:items-center sm:justify-between",
        className
      )}
    >
      <div className="text-sm text-slate-600">{label}</div>
      <div className="flex flex-wrap items-center justify-end gap-2">{children}</div>
    </div>
  );
}
