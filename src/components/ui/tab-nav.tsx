import * as React from "react";
import * as TabsPrimitive from "@radix-ui/react-tabs";
import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "@/lib/utils";

type TabNavVisualState = "content" | "loading" | "empty" | "error";

const tabsListVariants = cva(
  "inline-flex items-center justify-center rounded-md bg-muted p-1 text-muted-foreground",
  {
    variants: {
      orientation: {
        horizontal: "h-11 flex-row",
        vertical: "h-auto min-w-40 flex-col items-stretch",
      },
    },
    defaultVariants: {
      orientation: "horizontal",
    },
  },
);

const tabsTriggerVariants = cva(
  "inline-flex min-h-10 items-center justify-center rounded-sm px-3 py-2 text-sm font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:pointer-events-none disabled:opacity-50 data-[state=active]:bg-background data-[state=active]:text-foreground data-[state=active]:shadow-sm",
  {
    variants: {
      orientation: {
        horizontal: "whitespace-nowrap",
        vertical: "w-full justify-start",
      },
      state: {
        content: "",
        loading: "",
        empty: "",
        error: "data-[state=active]:text-destructive",
      },
    },
    defaultVariants: {
      orientation: "horizontal",
      state: "content",
    },
  },
);

interface TabNavProps extends React.ComponentPropsWithoutRef<typeof TabsPrimitive.Root> {
  orientation?: "horizontal" | "vertical";
  state?: TabNavVisualState;
}

const TabNav = React.forwardRef<
  React.ElementRef<typeof TabsPrimitive.Root>,
  TabNavProps
>(({ className, orientation = "horizontal", state = "content", ...props }, ref) => (
  <TabsPrimitive.Root
    ref={ref}
    className={cn(
      "flex gap-3",
      orientation === "vertical" ? "flex-row" : "flex-col",
      state === "error" ? "text-destructive" : "",
      className,
    )}
    orientation={orientation}
    {...props}
  />
));
TabNav.displayName = TabsPrimitive.Root.displayName;

interface TabNavListProps
  extends React.ComponentPropsWithoutRef<typeof TabsPrimitive.List>,
    VariantProps<typeof tabsListVariants> {}

const TabNavList = React.forwardRef<
  React.ElementRef<typeof TabsPrimitive.List>,
  TabNavListProps
>(({ className, orientation = "horizontal", ...props }, ref) => (
  <TabsPrimitive.List
    ref={ref}
    className={cn(tabsListVariants({ orientation }), className)}
    {...props}
  />
));
TabNavList.displayName = TabsPrimitive.List.displayName;

interface TabNavTriggerProps
  extends Omit<React.ComponentPropsWithoutRef<typeof TabsPrimitive.Trigger>, "orientation">,
    VariantProps<typeof tabsTriggerVariants> {}

const TabNavTrigger = React.forwardRef<
  React.ElementRef<typeof TabsPrimitive.Trigger>,
  TabNavTriggerProps
>(({ className, orientation = "horizontal", state = "content", children, ...props }, ref) => (
  <TabsPrimitive.Trigger
    ref={ref}
    className={cn(tabsTriggerVariants({ orientation, state }), className)}
    aria-busy={state === "loading" || undefined}
    aria-invalid={state === "error" || undefined}
    data-state-ui={state}
    {...props}
  >
    <span className="max-w-full truncate">{children}</span>
  </TabsPrimitive.Trigger>
));
TabNavTrigger.displayName = TabsPrimitive.Trigger.displayName;

const TabNavContent = React.forwardRef<
  React.ElementRef<typeof TabsPrimitive.Content>,
  React.ComponentPropsWithoutRef<typeof TabsPrimitive.Content>
>(({ className, ...props }, ref) => (
  <TabsPrimitive.Content
    ref={ref}
    className={cn("mt-2 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2", className)}
    {...props}
  />
));
TabNavContent.displayName = TabsPrimitive.Content.displayName;

export { TabNav, TabNavContent, TabNavList, TabNavTrigger };
