import * as React from "react";
import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "@/lib/utils";

const DONUT_RADIUS = 7;
const DONUT_CIRCUMFERENCE = 2 * Math.PI * DONUT_RADIUS;

const usageDonutToneVariants = cva("fill-none transition-all", {
  variants: {
    tone: {
      green: "stroke-green-500",
      yellow: "stroke-yellow-500",
      red: "stroke-red-500",
      neutral: "stroke-muted-foreground",
    },
  },
  defaultVariants: {
    tone: "neutral",
  },
});

const usageDonutSizeVariants = cva("shrink-0", {
  variants: {
    size: {
      sm: "h-4 w-4",
      md: "h-5 w-5",
      lg: "h-7 w-7",
    },
  },
  defaultVariants: {
    size: "md",
  },
});

function clampPercent(percent: number): number {
  if (!Number.isFinite(percent)) return 0;
  return Math.max(0, Math.min(100, percent));
}

export interface UsageDonutProps
  extends Omit<React.SVGProps<SVGSVGElement>, "aria-label">,
    VariantProps<typeof usageDonutToneVariants>,
    VariantProps<typeof usageDonutSizeVariants> {
  percent: number;
  "aria-label": string;
}

export const UsageDonut = React.forwardRef<SVGSVGElement, UsageDonutProps>(
  ({ className, tone = "neutral", size = "md", percent, "aria-label": ariaLabel, ...props }, ref) => {
    const clampedPercent = clampPercent(percent);
    const dashOffset = DONUT_CIRCUMFERENCE * (1 - clampedPercent / 100);

    return (
      <svg
        ref={ref}
        className={cn(usageDonutSizeVariants({ size }), className)}
        viewBox="0 0 20 20"
        role="img"
        aria-label={ariaLabel}
        {...props}
      >
        <circle cx="10" cy="10" r={DONUT_RADIUS} className="fill-none stroke-border" strokeWidth="3" />
        <circle
          cx="10"
          cy="10"
          r={DONUT_RADIUS}
          className={cn(usageDonutToneVariants({ tone }))}
          strokeWidth="3"
          strokeLinecap="round"
          strokeDasharray={`${DONUT_CIRCUMFERENCE} ${DONUT_CIRCUMFERENCE}`}
          strokeDashoffset={dashOffset}
          transform="rotate(-90 10 10)"
        />
      </svg>
    );
  },
);

UsageDonut.displayName = "UsageDonut";
