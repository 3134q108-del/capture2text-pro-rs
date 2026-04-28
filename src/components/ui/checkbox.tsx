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

export interface CheckboxProps
  extends Omit<React.ComponentPropsWithoutRef<typeof CheckboxPrimitive.Root>, "size">,
    VariantProps<typeof checkboxVariants> {
  label?: React.ReactNode;
  description?: React.ReactNode;
}

export const Checkbox = React.forwardRef<
  React.ElementRef<typeof CheckboxPrimitive.Root>,
  CheckboxProps
>(({ className, size, state = "content", disabled, label, description, id, ...props }, ref) => {
  const isLoading = state === "loading";
  const isEmpty = state === "empty";
  const resolvedDisabled = disabled || isLoading || isEmpty;

  return (
    <label className={cn("inline-flex min-h-11 items-start gap-3", className)}>
      <CheckboxPrimitive.Root
        ref={ref}
        id={id}
        disabled={resolvedDisabled}
        aria-busy={isLoading || undefined}
        aria-invalid={state === "error" || undefined}
        className={cn(checkboxVariants({ size, state }))}
        {...props}
      >
        <CheckboxPrimitive.Indicator className="flex items-center justify-center text-current">
          <span aria-hidden="true">{props.checked === "indeterminate" ? "—" : "✓"}</span>
        </CheckboxPrimitive.Indicator>
      </CheckboxPrimitive.Root>
      {(label || description) ? (
        <span className="flex min-w-0 flex-1 flex-col gap-1">
          {label ? <span className="text-sm font-medium leading-tight">{label}</span> : null}
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
