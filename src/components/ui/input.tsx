import * as React from "react";
import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "@/lib/utils";

type InputKind = "text" | "password" | "number" | "email" | "search";

const inputVariants = cva(
  "w-full rounded-md border bg-background text-foreground placeholder:text-muted-foreground transition-colors file:border-0 file:bg-transparent file:text-sm file:font-medium focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50",
  {
    variants: {
      size: {
        sm: "h-11 px-3 text-sm",
        md: "h-11 px-3 text-sm",
        lg: "h-12 px-4 text-base",
      },
      state: {
        content: "border-input",
        loading: "border-input opacity-90",
        empty: "border-input text-muted-foreground",
        error: "border-destructive",
      },
    },
    defaultVariants: {
      size: "md",
      state: "content",
    },
  },
);

export interface InputProps
  extends Omit<React.InputHTMLAttributes<HTMLInputElement>, "size" | "type">,
    VariantProps<typeof inputVariants> {
  type?: InputKind;
}

export const Input = React.forwardRef<HTMLInputElement, InputProps>(
  ({ className, size, state = "content", type = "text", disabled, readOnly, ...props }, ref) => {
    const isLoading = state === "loading";
    const resolvedDisabled = disabled || isLoading;
    const resolvedReadOnly = readOnly || isLoading;

    return (
      <input
        type={type}
        ref={ref}
        disabled={resolvedDisabled}
        readOnly={resolvedReadOnly}
        aria-busy={isLoading || undefined}
        aria-invalid={state === "error" || undefined}
        data-state={state}
        className={cn(inputVariants({ size, state }), className)}
        {...props}
      />
    );
  },
);

Input.displayName = "Input";
