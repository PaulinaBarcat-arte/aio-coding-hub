import type { ReactNode } from "react";
import { cn } from "../utils/cn";

export type FormFieldProps = {
  label: string;
  hint?: ReactNode;
  children: ReactNode;
  className?: string;
};

export function FormField({ label, hint, children, className }: FormFieldProps) {
  return (
    <div className={cn("space-y-1.5", className)}>
      <div className="flex items-center justify-between gap-3">
        <div className="text-sm font-medium text-slate-700">{label}</div>
        {hint ? <div className="text-xs text-slate-500">{hint}</div> : null}
      </div>
      {children}
    </div>
  );
}
