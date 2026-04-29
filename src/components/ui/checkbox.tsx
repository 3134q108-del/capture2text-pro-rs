import * as React from "react";
import * as CheckboxPrimitive from "@radix-ui/react-checkbox";
import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "@/lib/utils";
import { StatusText } from "./status-text";

const checkboxVariants = cva(
  "peer shrink-0 rounded-sm border border-input bg-background shadow-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50 data-[state=checked]:bg-primary data-[state=checked]:text-primary-foreground data-[state=indeterminate]:bg-primary data-[state=indeterminate]:text-primary-foreground",
  {
    variants: {
      size: {
        sm: "h-5 w-5",
        md: "h-6 w-6",
      },
      state: {
        content: "",
        loading: "",
        empty: "",
        error: "border-destructive",
      },
    },
    defaultVariants: {
      size: "md",
      state: "content",
    },
  },
);

const indicatorIconVariants = cva("text-current", {
  variants: {
    size: {
      sm: "h-3 w-3",
      md: "h-3.5 w-3.5",
    },
  },
  defaultVariants: {
    size: "md",
  },
});

export interface CheckboxProps
  extends Omit<React.ComponentPropsWithoutRef<typeof CheckboxPrimitive.Root>, "size">,
    VariantProps<typeof checkboxVariants> {
  label?: React.ReactNode;
  description?: React.ReactNode;
}

export const Checkbox = React.forwardRef<
  React.ElementRef<typeof CheckboxPrimitive.Root>,
  CheckboxProps
>(({ className, size = "md", state = "content", disabled, label, description, id, checked, ...props }, ref) => {
  const isLoading = state === "loading";
  const isEmpty = state === "empty";
  const resolvedDisabled = disabled || isLoading || isEmpty;
  const isIndeterminate = checked === "indeterminate";
  const hasDescription = Boolean(description);

  return (
    <label className={cn("inline-flex min-h-11 gap-3", hasDescription ? "items-start" : "items-center", className)}>
      <CheckboxPrimitive.Root
        ref={ref}
        id={id}
        checked={checked}
        disabled={resolvedDisabled}
        aria-busy={isLoading || undefined}
        aria-invalid={state === "error" || undefined}
        className={cn(checkboxVariants({ size, state }))}
        {...props}
      >
        <CheckboxPrimitive.Indicator className="flex items-center justify-center text-current">
          {isIndeterminate ? (
            <svg viewBox="0 0 16 16" className={cn(indicatorIconVariants({ size }))} aria-hidden="true">
              <path d="M3.5 8h9" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
            </svg>
          ) : (
            <svg viewBox="0 0 16 16" className={cn(indicatorIconVariants({ size }))} aria-hidden="true">
              <path d="M3.5 8.5 6.5 11.5 12.5 5.5" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" />
            </svg>
          )}
        </CheckboxPrimitive.Indicator>
      </CheckboxPrimitive.Root>
      {(label || description) ? (
        <span className={cn("flex min-w-0 flex-1", hasDescription ? "flex-col gap-1" : "items-center")}>
          {label ? <span className={cn("text-sm font-medium", hasDescription ? "leading-tight" : "leading-none")}>{label}</span> : null}
          {description ? (
            <StatusText tone={state === "error" ? "error" : "info"} size="sm" state={state === "error" ? "error" : "content"}>
              {description}
            </StatusText>
          ) : null}
        </span>
      ) : null}
    </label>
  );
});
Checkbox.displayName = "Checkbox";
