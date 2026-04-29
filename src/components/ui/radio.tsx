import * as React from "react";
import * as RadioGroupPrimitive from "@radix-ui/react-radio-group";
import { cva, type VariantProps } from "class-variance-authority";
import { Circle } from "lucide-react";
import { cn } from "@/lib/utils";
import { StatusText } from "./status-text";

type RadioVisualState = "content" | "loading" | "empty" | "error";

const radioGroupVariants = cva("grid gap-2", {
  variants: {
    orientation: {
      horizontal: "grid-flow-col auto-cols-fr",
      vertical: "grid-flow-row",
    },
    state: {
      content: "",
      loading: "opacity-90",
      empty: "opacity-80",
      error: "",
    },
  },
  defaultVariants: {
    orientation: "vertical",
    state: "content",
  },
});

const radioItemVariants = cva(
  "inline-flex aspect-square shrink-0 items-center justify-center rounded-full border border-input text-primary shadow-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50 data-[state=checked]:border-primary",
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

const radioIndicatorVariants = cva("fill-current stroke-0 text-primary", {
  variants: {
    size: {
      sm: "h-1.5 w-1.5",
      md: "h-2 w-2",
    },
  },
  defaultVariants: {
    size: "md",
  },
});

interface RadioGroupProps extends React.ComponentPropsWithoutRef<typeof RadioGroupPrimitive.Root> {
  orientation?: "horizontal" | "vertical";
  state?: RadioVisualState;
}

const RadioGroup = React.forwardRef<
  React.ElementRef<typeof RadioGroupPrimitive.Root>,
  RadioGroupProps
>(({ className, orientation, state = "content", disabled, ...props }, ref) => {
  const isLoading = state === "loading";
  const isEmpty = state === "empty";
  const resolvedDisabled = disabled || isLoading || isEmpty;

  return (
    <RadioGroupPrimitive.Root
      ref={ref}
      className={cn(radioGroupVariants({ orientation, state }), className)}
      orientation={orientation === "horizontal" ? "horizontal" : "vertical"}
      aria-invalid={state === "error" || undefined}
      aria-busy={isLoading || undefined}
      disabled={resolvedDisabled}
      {...props}
    />
  );
});
RadioGroup.displayName = RadioGroupPrimitive.Root.displayName;

interface RadioGroupItemProps
  extends Omit<React.ComponentPropsWithoutRef<typeof RadioGroupPrimitive.Item>, "size">,
    VariantProps<typeof radioItemVariants> {
  label?: React.ReactNode;
  description?: React.ReactNode;
}

const RadioGroupItem = React.forwardRef<
  React.ElementRef<typeof RadioGroupPrimitive.Item>,
  RadioGroupItemProps
>(({ className, size = "md", state = "content", label, description, id, ...props }, ref) => (
  <label className={cn("inline-flex min-h-11 items-start gap-3", className)}>
    <RadioGroupPrimitive.Item ref={ref} id={id} className={cn(radioItemVariants({ size, state }))} {...props}>
      <RadioGroupPrimitive.Indicator className="flex items-center justify-center">
        <Circle className={cn(radioIndicatorVariants({ size }))} aria-hidden="true" />
      </RadioGroupPrimitive.Indicator>
    </RadioGroupPrimitive.Item>
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
));
RadioGroupItem.displayName = RadioGroupPrimitive.Item.displayName;

export { RadioGroup, RadioGroupItem };
