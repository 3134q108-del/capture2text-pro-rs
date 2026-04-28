import * as React from "react";
import * as PopoverPrimitive from "@radix-ui/react-popover";
import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "@/lib/utils";

type PopoverVisualState = "content" | "loading" | "empty" | "error";

const popoverContentVariants = cva(
  "z-50 rounded-md border bg-popover p-3 text-popover-foreground shadow-md outline-none",
  {
    variants: {
      size: {
        sm: "w-56",
        md: "w-72",
        lg: "w-80",
      },
      state: {
        content: "",
        loading: "opacity-90",
        empty: "text-muted-foreground",
        error: "border-destructive",
      },
    },
    defaultVariants: {
      size: "md",
      state: "content",
    },
  },
);

const Popover = PopoverPrimitive.Root;
const PopoverTrigger = PopoverPrimitive.Trigger;
const PopoverAnchor = PopoverPrimitive.Anchor;

interface PopoverContentProps
  extends React.ComponentPropsWithoutRef<typeof PopoverPrimitive.Content>,
    VariantProps<typeof popoverContentVariants> {
  state?: PopoverVisualState;
  loadingContent?: React.ReactNode;
  emptyContent?: React.ReactNode;
  errorContent?: React.ReactNode;
}

const PopoverContent = React.forwardRef<
  React.ElementRef<typeof PopoverPrimitive.Content>,
  PopoverContentProps
>(({ className, size, state = "content", sideOffset = 8, collisionPadding = 8, children, loadingContent, emptyContent, errorContent, ...props }, ref) => {
  const resolvedContent =
    state === "error"
      ? errorContent ?? children
      : state === "loading"
        ? loadingContent ?? children
        : state === "empty"
          ? emptyContent ?? children
          : children;

  return (
    <PopoverPrimitive.Portal>
      <PopoverPrimitive.Content
        ref={ref}
        className={cn(popoverContentVariants({ size, state }), className)}
        sideOffset={sideOffset}
        collisionPadding={collisionPadding}
        aria-busy={state === "loading" || undefined}
        aria-invalid={state === "error" || undefined}
        data-state-ui={state}
        {...props}
      >
        <div className="max-h-screen overflow-y-auto">{resolvedContent}</div>
      </PopoverPrimitive.Content>
    </PopoverPrimitive.Portal>
  );
});
PopoverContent.displayName = PopoverPrimitive.Content.displayName;

export { Popover, PopoverAnchor, PopoverContent, PopoverTrigger };
