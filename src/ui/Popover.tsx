import { useCallback, useEffect, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { cn } from "../utils/cn";

export type PopoverProps = {
  trigger: ReactNode;
  children: ReactNode;
  open?: boolean;
  onOpenChange?: (open: boolean) => void;
  placement?: "top" | "bottom";
  align?: "start" | "center" | "end";
  className?: string;
  contentClassName?: string;
};

type PopoverPosition = {
  left: number;
  top: number;
};

export function Popover({
  trigger,
  children,
  open: controlledOpen,
  onOpenChange,
  placement = "bottom",
  align = "end",
  className,
  contentClassName,
}: PopoverProps) {
  const anchorRef = useRef<HTMLButtonElement>(null);
  const contentRef = useRef<HTMLDivElement>(null);
  const [internalOpen, setInternalOpen] = useState(false);
  const [pos, setPos] = useState<PopoverPosition | null>(null);

  const isControlled = controlledOpen !== undefined;
  const open = isControlled ? controlledOpen : internalOpen;

  const setOpen = useCallback(
    (next: boolean) => {
      if (!isControlled) setInternalOpen(next);
      onOpenChange?.(next);
    },
    [isControlled, onOpenChange]
  );

  const updatePosition = useCallback(() => {
    const el = anchorRef.current;
    if (!el) return;
    const rect = el.getBoundingClientRect();

    let left: number;
    if (align === "start") {
      left = rect.left;
    } else if (align === "center") {
      left = rect.left + rect.width / 2;
    } else {
      left = rect.right;
    }

    setPos({
      left,
      top: placement === "bottom" ? rect.bottom : rect.top,
    });
  }, [placement, align]);

  useLayoutEffect(() => {
    if (!open) return;
    updatePosition();

    const onScrollOrResize = () => updatePosition();
    window.addEventListener("scroll", onScrollOrResize, true);
    window.addEventListener("resize", onScrollOrResize);
    return () => {
      window.removeEventListener("scroll", onScrollOrResize, true);
      window.removeEventListener("resize", onScrollOrResize);
    };
  }, [open, updatePosition]);

  useEffect(() => {
    if (open && pos && contentRef.current) {
      contentRef.current.focus();
    }
  }, [open, pos]);

  useEffect(() => {
    if (!open) return;

    const handleClickOutside = (e: MouseEvent) => {
      const target = e.target as Node;
      if (
        contentRef.current &&
        !contentRef.current.contains(target) &&
        anchorRef.current &&
        !anchorRef.current.contains(target)
      ) {
        setOpen(false);
      }
    };

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        setOpen(false);
      }
    };

    window.addEventListener("mousedown", handleClickOutside);
    window.addEventListener("keydown", handleKeyDown);
    return () => {
      window.removeEventListener("mousedown", handleClickOutside);
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, [open, setOpen]);

  if (typeof document === "undefined") {
    return <>{trigger}</>;
  }

  const getTransform = () => {
    const yTransform = placement === "bottom" ? "8px" : "calc(-100% - 8px)";
    if (align === "start") return `translateY(${yTransform})`;
    if (align === "center") return `translate(-50%, ${yTransform})`;
    return `translate(-100%, ${yTransform})`;
  };

  return (
    <>
      <button
        ref={anchorRef}
        type="button"
        onClick={() => setOpen(!open)}
        className={cn("inline-flex", className)}
        aria-haspopup="dialog"
        aria-expanded={open}
      >
        {trigger}
      </button>
      {open && pos
        ? createPortal(
            <div
              ref={contentRef}
              className="fixed z-50 outline-none"
              tabIndex={-1}
              style={{
                left: pos.left,
                top: pos.top,
                transform: getTransform(),
              }}
              role="dialog"
              aria-modal="false"
            >
              <div className={cn("shadow-lg", contentClassName)}>{children}</div>
            </div>,
            document.body
          )
        : null}
    </>
  );
}
