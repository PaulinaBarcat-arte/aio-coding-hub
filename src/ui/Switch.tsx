import { cn } from "../utils/cn";

export type SwitchProps = {
  checked: boolean;
  onCheckedChange: (checked: boolean) => void;
  disabled?: boolean;
  size?: "sm" | "md";
  className?: string;
};

export function Switch({
  checked,
  onCheckedChange,
  disabled,
  size = "md",
  className,
}: SwitchProps) {
  const isSmall = size === "sm";
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      disabled={disabled}
      onClick={() => onCheckedChange(!checked)}
      className={cn(
        "inline-flex shrink-0 items-center rounded-full border-2 border-transparent transition-colors",
        "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[#0052FF]/30 focus-visible:ring-offset-2 focus-visible:ring-offset-[#FAFAFA]",
        isSmall ? "h-5 w-9" : "h-6 w-11",
        checked ? "bg-[#0052FF]" : "bg-slate-200",
        disabled ? "cursor-not-allowed opacity-50" : "cursor-pointer",
        className
      )}
    >
      <span
        className={cn(
          "pointer-events-none block rounded-full bg-white shadow-sm transition-transform",
          isSmall ? "h-4 w-4" : "h-5 w-5",
          checked ? (isSmall ? "translate-x-4" : "translate-x-5") : "translate-x-0"
        )}
      />
    </button>
  );
}
