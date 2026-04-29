import * as React from "react";
import { Slot } from "@radix-ui/react-slot";
import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "@/lib/utils";

const buttonVariants = cva(
  "inline-flex items-center justify-center gap-2 whitespace-nowrap rounded-md text-sm font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:pointer-events-none disabled:opacity-50",
  {
    variants: {
      variant: {
        primary: "bg-primary text-primary-foreground hover:bg-primary/90",
        secondary: "bg-secondary text-secondary-foreground hover:bg-accent hover:text-accent-foreground",
        ghost: "bg-transparent text-foreground hover:bg-accent hover:text-accent-foreground",
        destructive: "bg-destructive text-destructive-foreground hover:bg-destructive/90",
      },
      size: {
        sm: "h-7 px-2.5 text-xs",
        md: "h-8 px-3 text-sm",
        lg: "h-9 px-4 text-sm",
      },
      state: {
        content: "",
        loading: "cursor-wait",
        empty: "text-muted-foreground",
        error: "ring-1 ring-destructive",
      },
    },
    defaultVariants: {
      variant: "primary",
      size: "md",
      state: "content",
    },
  },
);

export interface ButtonProps
  extends Omit<React.ButtonHTMLAttributes<HTMLButtonElement>, "disabled">,
    VariantProps<typeof buttonVariants> {
  asChild?: boolean;
  disabled?: boolean;
  emptyContent?: React.ReactNode;
  loadingContent?: React.ReactNode;
  errorContent?: React.ReactNode;
}

function Spinner() {
  return <span className="h-4 w-4 animate-spin rounded-full border-2 border-current border-t-transparent" aria-hidden="true" />;
}

export const Button = React.forwardRef<HTMLButtonElement, ButtonProps>(
  (
    {
      className,
      variant,
      size,
      state = "content",
      asChild = false,
      disabled,
      children,
      emptyContent,
      loadingContent,
      errorContent,
      ...props
    },
    ref,
  ) => {
    const Comp = asChild ? Slot : "button";
    const isLoading = state === "loading";
    const isDisabled = disabled || isLoading;

    const resolvedContent =
      state === "error"
        ? errorContent ?? children
        : state === "loading"
          ? (
              <>
                <Spinner />
                <span className="truncate">{loadingContent ?? children}</span>
              </>
            )
          : state === "empty"
            ? emptyContent ?? children
            : children;

    return (
      <Comp
        className={cn(buttonVariants({ variant, size, state }), className)}
        data-loading={isLoading ? "" : undefined}
        aria-busy={isLoading || undefined}
        disabled={!asChild ? isDisabled : undefined}
        aria-disabled={asChild ? isDisabled : undefined}
        ref={ref}
        {...props}
      >
        <span className="max-w-full truncate">{resolvedContent}</span>
      </Comp>
    );
  },
);

Button.displayName = "Button";
