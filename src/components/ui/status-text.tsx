import * as React from "react";
import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "@/lib/utils";

const statusTextVariants = cva("inline-flex max-w-full items-start gap-1 break-words", {
  variants: {
    tone: {
      info: "text-muted-foreground",
      success: "text-green-700",
      warning: "text-yellow-700",
      error: "text-destructive",
    },
    size: {
      sm: "text-xs",
      md: "text-sm",
    },
    state: {
      content: "",
      loading: "opacity-80",
      empty: "hidden",
      error: "font-medium",
    },
  },
  defaultVariants: {
    tone: "info",
    size: "sm",
    state: "content",
  },
});

export interface StatusTextProps
  extends React.HTMLAttributes<HTMLParagraphElement>,
    VariantProps<typeof statusTextVariants> {
  loadingContent?: React.ReactNode;
  emptyContent?: React.ReactNode;
  errorContent?: React.ReactNode;
}

export const StatusText = React.forwardRef<HTMLParagraphElement, StatusTextProps>(
  (
    { className, tone, size, state = "content", children, loadingContent, emptyContent, errorContent, role, ...props },
    ref,
  ) => {
    if (state === "empty" && emptyContent == null) {
      return null;
    }

    const resolvedContent =
      state === "error"
        ? errorContent ?? children
        : state === "loading"
          ? loadingContent ?? children
          : state === "empty"
            ? emptyContent
            : children;

    const resolvedRole = role ?? (state === "error" || tone === "error" ? "alert" : "status");

    return (
      <p ref={ref} role={resolvedRole} className={cn(statusTextVariants({ tone, size, state }), className)} data-state={state} {...props}>
        {resolvedContent}
      </p>
    );
  },
);

StatusText.displayName = "StatusText";
