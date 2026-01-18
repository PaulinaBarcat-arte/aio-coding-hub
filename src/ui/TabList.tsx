import { Button, type ButtonSize } from "./Button";
import { cn } from "../utils/cn";

export type TabListItem<T extends string> = {
  key: T;
  label: string;
  disabled?: boolean;
};

export type TabListProps<T extends string> = {
  ariaLabel: string;
  items: Array<TabListItem<T>>;
  value: T;
  onChange: (next: T) => void;
  className?: string;
  size?: ButtonSize;
  buttonClassName?: string;
};

export function TabList<T extends string>({
  ariaLabel,
  items,
  value,
  onChange,
  className,
  size = "sm",
  buttonClassName,
}: TabListProps<T>) {
  return (
    <div
      role="tablist"
      aria-label={ariaLabel}
      className={cn("flex items-center rounded-lg border border-slate-200 bg-white p-1", className)}
    >
      {items.map((item) => {
        const active = value === item.key;
        return (
          <Button
            key={item.key}
            onClick={() => onChange(item.key)}
            variant={active ? "primary" : "ghost"}
            size={size}
            role="tab"
            aria-selected={active}
            disabled={item.disabled}
            className={cn("h-auto px-3 py-2 shadow-none", buttonClassName)}
          >
            <span className="text-sm font-semibold">{item.label}</span>
          </Button>
        );
      })}
    </div>
  );
}
