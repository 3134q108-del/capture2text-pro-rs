import * as React from "react";
import * as SliderPrimitive from "@radix-ui/react-slider";
import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "@/lib/utils";

type SliderVisualState = "content" | "loading" | "empty" | "error";

const sliderTrackVariants = cva("relative grow overflow-hidden rounded-full bg-muted", {
  variants: {
    size: {
      sm: "h-1.5",
      md: "h-2",
    },
  },
  defaultVariants: {
    size: "md",
  },
});

const sliderThumbVariants = cva(
  "block rounded-full border border-input bg-background shadow-sm transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:pointer-events-none disabled:opacity-50",
  {
    variants: {
      size: {
        sm: "h-4 w-4",
        md: "h-5 w-5",
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

interface SliderProps
  extends Omit<React.ComponentPropsWithoutRef<typeof SliderPrimitive.Root>, "size">,
    VariantProps<typeof sliderThumbVariants> {
  state?: SliderVisualState;
}

const Slider = React.forwardRef<
  React.ElementRef<typeof SliderPrimitive.Root>,
  SliderProps
>(({ className, size, state = "content", value, min = 0, max = 100, step = 1, disabled, ...props }, ref) => {
  const isLoading = state === "loading";
  const isEmpty = state === "empty";
  const resolvedDisabled = disabled || isLoading || isEmpty;
  const resolvedValue = value && value.length > 0 ? value : [min];

  return (
    <SliderPrimitive.Root
      ref={ref}
      value={resolvedValue}
      min={min}
      max={max}
      step={step}
      disabled={resolvedDisabled}
      aria-busy={isLoading || undefined}
      aria-invalid={state === "error" || undefined}
      className={cn("relative flex w-full touch-none select-none items-center", className)}
      {...props}
    >
      <SliderPrimitive.Track className={cn(sliderTrackVariants({ size }))}>
        <SliderPrimitive.Range className="absolute h-full bg-primary" />
      </SliderPrimitive.Track>
      {resolvedValue.map((_, index) => (
        <SliderPrimitive.Thumb key={index} className={cn(sliderThumbVariants({ size, state }))} />
      ))}
    </SliderPrimitive.Root>
  );
});
Slider.displayName = SliderPrimitive.Root.displayName;

export { Slider };
