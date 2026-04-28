import * as React from "react";
import { cva } from "class-variance-authority";
import { cn } from "@/lib/utils";

type CardState = "content" | "loading" | "empty" | "error";

const cardVariants = cva("rounded-lg border bg-card text-card-foreground shadow-sm", {
  variants: {
    state: {
      content: "",
      loading: "opacity-90",
      empty: "text-muted-foreground",
      error: "border-destructive",
    },
  },
  defaultVariants: {
    state: "content",
  },
});

type CardElementProps = React.HTMLAttributes<HTMLDivElement>;

export interface CardProps extends CardElementProps {
  state?: CardState;
  loadingContent?: React.ReactNode;
  emptyContent?: React.ReactNode;
  errorContent?: React.ReactNode;
}

function resolveCardContent({
  state,
  children,
  loadingContent,
  emptyContent,
  errorContent,
}: Pick<CardProps, "state" | "children" | "loadingContent" | "emptyContent" | "errorContent">) {
  if (state === "error") return errorContent ?? children;
  if (state === "loading") return loadingContent ?? children;
  if (state === "empty") return emptyContent ?? children;
  return children;
}

const Card = React.forwardRef<HTMLDivElement, CardProps>(
  (
    { className, state = "content", children, loadingContent, emptyContent, errorContent, ...props },
    ref,
  ) => (
    <div ref={ref} className={cn(cardVariants({ state }), className)} data-state={state} {...props}>
      {resolveCardContent({ state, children, loadingContent, emptyContent, errorContent })}
    </div>
  ),
);
Card.displayName = "Card";

const CardHeader = React.forwardRef<HTMLDivElement, CardElementProps>(({ className, ...props }, ref) => (
  <div ref={ref} className={cn("flex flex-col gap-1.5 p-4", className)} {...props} />
));
CardHeader.displayName = "CardHeader";

const CardTitle = React.forwardRef<HTMLHeadingElement, React.HTMLAttributes<HTMLHeadingElement>>(
  ({ className, ...props }, ref) => (
    <h3 ref={ref} className={cn("truncate text-base font-semibold leading-none tracking-tight", className)} {...props} />
  ),
);
CardTitle.displayName = "CardTitle";

const CardDescription = React.forwardRef<HTMLParagraphElement, React.HTMLAttributes<HTMLParagraphElement>>(
  ({ className, ...props }, ref) => (
    <p ref={ref} className={cn("text-sm text-muted-foreground", className)} {...props} />
  ),
);
CardDescription.displayName = "CardDescription";

const CardContent = React.forwardRef<HTMLDivElement, CardElementProps>(({ className, ...props }, ref) => (
  <div ref={ref} className={cn("p-4 pt-0", className)} {...props} />
));
CardContent.displayName = "CardContent";

const CardFooter = React.forwardRef<HTMLDivElement, CardElementProps>(({ className, ...props }, ref) => (
  <div ref={ref} className={cn("flex items-center gap-2 p-4 pt-0", className)} {...props} />
));
CardFooter.displayName = "CardFooter";

export { Card, CardContent, CardDescription, CardFooter, CardHeader, CardTitle };
