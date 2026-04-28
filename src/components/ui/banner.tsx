import * as React from "react";
import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "@/lib/utils";

const bannerVariants = cva("flex w-full items-start gap-3 rounded-md border", {
  variants: {
    tone: {
      info: "border-border bg-muted text-foreground",
      success: "border-green-300 bg-green-50 text-green-900",
      warning: "border-yellow-300 bg-yellow-50 text-yellow-900",
      destructive: "border-destructive/40 bg-red-50 text-destructive",
    },
    size: {
      sm: "p-3",
      md: "p-4",
    },
    state: {
      content: "",
      loading: "opacity-90",
      empty: "text-muted-foreground",
      error: "",
    },
  },
  defaultVariants: {
    tone: "info",
    size: "md",
    state: "content",
  },
});

const bannerTitleVariants = cva("font-medium", {
  variants: {
    size: {
      sm: "text-sm",
      md: "text-base",
    },
  },
  defaultVariants: {
    size: "md",
  },
});

const bannerDescriptionVariants = cva("break-words", {
  variants: {
    size: {
      sm: "text-xs",
      md: "text-sm",
    },
  },
  defaultVariants: {
    size: "md",
  },
});

export interface BannerProps
  extends Omit<React.HTMLAttributes<HTMLDivElement>, "title" | "children">,
    VariantProps<typeof bannerVariants> {
  title?: React.ReactNode;
  description: React.ReactNode;
  icon?: React.ReactNode;
  action?: React.ReactNode;
  onDismiss?: () => void;
  disabled?: boolean;
  loadingDescription?: React.ReactNode;
  emptyDescription?: React.ReactNode;
  errorDescription?: React.ReactNode;
}

export const Banner = React.forwardRef<HTMLDivElement, BannerProps>(
  (
    {
      className,
      tone = "info",
      size = "md",
      state = "content",
      title,
      description,
      icon,
      action,
      onDismiss,
      disabled = false,
      loadingDescription,
      emptyDescription,
      errorDescription,
      role,
      ...props
    },
    ref,
  ) => {
    if (state === "empty" && emptyDescription == null && description == null) {
      return null;
    }

    const resolvedDescription =
      state === "error"
        ? errorDescription ?? description
        : state === "loading"
          ? loadingDescription ?? description
          : state === "empty"
            ? emptyDescription ?? description
            : description;
    const resolvedRole = role ?? (state === "error" || tone === "destructive" ? "alert" : "status");

    return (
      <div ref={ref} role={resolvedRole} className={cn(bannerVariants({ tone, size, state }), className)} data-state={state} {...props}>
        {icon ? <span className="mt-0.5 shrink-0" aria-hidden="true">{icon}</span> : null}

        <div className="flex min-w-0 flex-1 flex-col gap-1">
          {title ? <p className={cn(bannerTitleVariants({ size }))}>{title}</p> : null}
          <p className={cn(bannerDescriptionVariants({ size }))}>{resolvedDescription}</p>
          {action ? <div className="pt-1">{action}</div> : null}
        </div>

        {onDismiss ? (
          <button
            type="button"
            className="inline-flex h-11 w-11 shrink-0 items-center justify-center rounded-md border border-transparent text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:opacity-50"
            aria-label="Dismiss banner"
            onClick={onDismiss}
            disabled={disabled}
          >
            <svg viewBox="0 0 16 16" className="h-4 w-4" aria-hidden="true">
              <path d="M4 4l8 8M12 4l-8 8" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
            </svg>
          </button>
        ) : null}
      </div>
    );
  },
);

Banner.displayName = "Banner";
