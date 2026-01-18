import { useCallback, useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { cn } from "../utils/cn";

export type TooltipProps = {
  content: string;
  children: React.ReactNode;
  className?: string;
  contentClassName?: string;
  placement?: "top" | "bottom";
};

type TooltipPosition = {
  left: number;
  top: number;
};

export function Tooltip({
  content,
  children,
  className,
  contentClassName,
  placement = "top",
}: TooltipProps) {
  const anchorRef = useRef<HTMLSpanElement>(null);
  const [open, setOpen] = useState(false);
  const [pos, setPos] = useState<TooltipPosition | null>(null);

  const update = useCallback(() => {
    const el = anchorRef.current;
    if (!el) return;
    const rect = el.getBoundingClientRect();
    setPos({
      left: rect.left + rect.width / 2,
      top: placement === "bottom" ? rect.bottom : rect.top,
    });
  }, [placement]);

  useEffect(() => {
    if (!open) return;

    update();

    const onScrollOrResize = () => update();
    window.addEventListener("scroll", onScrollOrResize, true);
    window.addEventListener("resize", onScrollOrResize);
    return () => {
      window.removeEventListener("scroll", onScrollOrResize, true);
      window.removeEventListener("resize", onScrollOrResize);
    };
  }, [open, update]);

  if (typeof document === "undefined") {
    return <>{children}</>;
  }

  return (
    <>
      <span
        ref={anchorRef}
        className={cn("inline-flex", className)}
        onMouseEnter={() => setOpen(true)}
        onMouseLeave={() => setOpen(false)}
      >
        {children}
      </span>
      {open && pos
        ? createPortal(
            <div
              className="pointer-events-none fixed z-50"
              style={{
                left: pos.left,
                top: pos.top,
                transform:
                  placement === "bottom"
                    ? "translate(-50%, 8px)"
                    : "translate(-50%, calc(-100% - 8px))",
              }}
              role="tooltip"
              aria-hidden="true"
            >
              <div
                className={cn(
                  "max-w-[280px] whitespace-normal rounded-lg bg-slate-900 px-2 py-1 text-xs leading-snug text-white shadow-lg",
                  contentClassName
                )}
              >
                {content}
              </div>
            </div>,
            document.body
          )
        : null}
    </>
  );
}
