import { useEffect } from "react";
import { createPortal } from "react-dom";
import { cn } from "../utils/cn";

export type DialogProps = {
  open: boolean;
  title: string;
  description?: string;
  onOpenChange: (open: boolean) => void;
  children: React.ReactNode;
  className?: string;
};

export function Dialog({
  open,
  title,
  description,
  onOpenChange,
  children,
  className,
}: DialogProps) {
  useEffect(() => {
    if (!open) return;
    function onKeyDown(e: KeyboardEvent) {
      if (e.key === "Escape") onOpenChange(false);
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [open, onOpenChange]);

  if (!open) return null;

  return createPortal(
    <div className="fixed inset-0 z-50">
      <div className="absolute inset-0 bg-black/30" onClick={() => onOpenChange(false)} />
      <div className="absolute inset-0 flex items-center justify-center p-4">
        <div
          className={cn(
            "w-full max-w-3xl overflow-hidden rounded-2xl border border-slate-200 bg-white shadow-card",
            "flex max-h-[calc(100vh-2rem)] flex-col",
            className
          )}
          role="dialog"
          aria-modal="true"
        >
          <div className="flex items-start justify-between gap-4 border-b border-slate-200 px-5 py-4">
            <div className="min-w-0">
              <div className="truncate text-sm font-semibold">{title}</div>
              {description ? (
                <div className="mt-1 text-xs text-slate-500">{description}</div>
              ) : null}
            </div>

            <button
              type="button"
              className="rounded-lg border border-slate-200 bg-white px-2 py-1 text-xs text-slate-600 hover:bg-slate-50"
              onClick={() => onOpenChange(false)}
              aria-label="关闭"
            >
              关闭
            </button>
          </div>

          <div className="min-h-0 flex-1 overflow-y-auto px-5 py-4">{children}</div>
        </div>
      </div>
    </div>,
    document.body
  );
}
