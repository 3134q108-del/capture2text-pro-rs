import * as React from "react";
import * as SwitchPrimitive from "@radix-ui/react-switch";
import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "@/lib/utils";
import { StatusText } from "./status-text";

const toggleRootVariants = cva("inline-flex items-start gap-3", {
  variants: {
    state: {
      content: "",
      loading: "opacity-90",
      empty: "text-muted-foreground",
      error: "",
    },
  },
  defaultVariants: {
    state: "content",
  },
});

const toggleSwitchVariants = cva(
  "peer inline-flex shrink-0 cursor-pointer items-center rounded-full border border-input transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:pointer-events-none disabled:opacity-50 data-[state=checked]:bg-primary data-[state=unchecked]:bg-muted",
  {
    variants: {
      size: {
        sm: "h-6 w-11",
        md: "h-6 w-12",
        lg: "h-7 w-14",
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

const toggleThumbVariants = cva("pointer-events-none block rounded-full bg-background shadow-sm ring-0 transition-transform", {
  variants: {
    size: {
      sm: "h-5 w-5 data-[state=checked]:translate-x-5 data-[state=unchecked]:translate-x-0",
      md: "h-5 w-5 data-[state=checked]:translate-x-6 data-[state=unchecked]:translate-x-0",
      lg: "h-6 w-6 data-[state=checked]:translate-x-7 data-[state=unchecked]:translate-x-0",
    },
  },
  defaultVariants: {
    size: "md",
  },
});

export interface ToggleProps
  extends Omit<React.ComponentPropsWithoutRef<typeof SwitchPrimitive.Root>, "size">,
    VariantProps<typeof toggleSwitchVariants> {
  label?: React.ReactNode;
  description?: React.ReactNode;
}

export const Toggle = React.forwardRef<
  React.ElementRef<typeof SwitchPrimitive.Root>,
  ToggleProps
>(({ className, size, state = "content", checked, disabled, label, description, id, ...props }, ref) => {
  const isLoading = state === "loading";
  const isEmpty = state === "empty";
  const resolvedDisabled = disabled || isLoading || isEmpty;
  const hasText = label != null || description != null;
  const statusTone = state === "error" ? "error" : "info";

  return (
    <label className={cn(toggleRootVariants({ state }), "min-h-11", className)}>
      <SwitchPrimitive.Root
        ref={ref}
        id={id}
        checked={checked}
        disabled={resolvedDisabled}
        aria-busy={isLoading || undefined}
        aria-invalid={state === "error" || undefined}
        className={cn(toggleSwitchVariants({ size, state }))}
        {...props}
      >
        <SwitchPrimitive.Thumb className={cn(toggleThumbVariants({ size }))} />
      </SwitchPrimitive.Root>
      {hasText ? (
        <span className="flex min-w-0 flex-1 flex-col gap-1">
          {label ? <span className="text-sm font-medium leading-tight">{label}</span> : null}
          {description ? (
            <StatusText tone={statusTone} size="sm" state={state === "error" ? "error" : "content"}>
              {description}
            </StatusText>
          ) : null}
        </span>
      ) : null}
    </label>
  );
});
Toggle.displayName = "Toggle";
