import * as React from "react";
import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "@/lib/utils";
import { StatusText } from "./status-text";

type ProgressState = "content" | "loading" | "empty" | "error";

const progressTrackVariants = cva("w-full overflow-hidden rounded-full border border-border bg-muted", {
  variants: {
    size: {
      sm: "h-2",
      md: "h-3",
    },
  },
  defaultVariants: {
    size: "md",
  },
});

const progressBarVariants = cva("w-full rounded-full", {
  variants: {
    tone: {
      green: "accent-green-500",
      yellow: "accent-yellow-500",
      red: "accent-destructive",
      neutral: "accent-muted-foreground",
    },
    size: {
      sm: "h-2",
      md: "h-3",
    },
    state: {
      content: "",
      loading: "animate-pulse",
      empty: "opacity-70",
      error: "",
    },
    disabled: {
      true: "opacity-50",
      false: "",
    },
  },
  defaultVariants: {
    tone: "neutral",
    size: "md",
    state: "content",
    disabled: false,
  },
});

export interface ProgressBarProps
  extends Omit<React.HTMLAttributes<HTMLDivElement>, "children">,
    Omit<VariantProps<typeof progressBarVariants>, "disabled"> {
  value: number;
  max?: number;
  label?: React.ReactNode;
  subLabel?: React.ReactNode;
  disabled?: boolean;
}

function clampPercent(value: number): number {
  if (!Number.isFinite(value)) return 0;
  return Math.max(0, Math.min(100, value));
}

export const ProgressBar = React.forwardRef<HTMLDivElement, ProgressBarProps>(
  (
    { className, tone = "neutral", size = "md", state = "content", value, max = 100, label, subLabel, disabled = false, ...props },
    ref,
  ) => {
    const safeMax = Number.isFinite(max) && max > 0 ? max : 100;
    const computedPercent = clampPercent((value / safeMax) * 100);
    const resolvedState: ProgressState =
      state === "error"
        ? "error"
        : state === "loading"
          ? "loading"
          : state === "empty"
            ? "empty"
            : "content";
    const displayValue = resolvedState === "empty" ? 0 : computedPercent;
    const resolvedTone = resolvedState === "error" ? "red" : tone;

    return (
      <div ref={ref} className={cn("flex w-full flex-col gap-1.5", className)} data-state={resolvedState} {...props}>
        {(label || subLabel) ? (
          <div className="flex items-center justify-between gap-2">
            {label ? <span className="truncate text-sm font-medium text-foreground">{label}</span> : <span />}
            {subLabel ? <span className="truncate text-xs text-muted-foreground">{subLabel}</span> : null}
          </div>
        ) : null}

        <div className={cn(progressTrackVariants({ size }))}>
          <progress
            className={cn(progressBarVariants({ tone: resolvedTone, size, state: resolvedState, disabled }))}
            value={displayValue}
            max={100}
            role="progressbar"
            aria-valuemin={0}
            aria-valuemax={100}
            aria-valuenow={Math.round(displayValue)}
            aria-busy={resolvedState === "loading" || undefined}
            aria-invalid={resolvedState === "error" || undefined}
            aria-disabled={disabled || undefined}
          />
        </div>

        {resolvedState === "error" ? (
          <StatusText tone="error" size="sm" state="error">
            Failed to load progress
          </StatusText>
        ) : null}
      </div>
    );
  },
);

ProgressBar.displayName = "ProgressBar";
