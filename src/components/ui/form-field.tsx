import * as React from "react";
import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "@/lib/utils";
import { StatusText } from "./status-text";

type FormFieldRenderState = "content" | "loading" | "empty" | "error";

const formFieldVariants = cva("grid gap-2", {
  variants: {
    orientation: {
      vertical: "grid-cols-1",
      horizontal: "grid-cols-1 md:grid-cols-2 md:items-start md:gap-4",
    },
    state: {
      default: "",
      error: "",
    },
  },
  defaultVariants: {
    orientation: "vertical",
    state: "default",
  },
});

export interface FormFieldProps
  extends React.HTMLAttributes<HTMLDivElement>,
    VariantProps<typeof formFieldVariants> {
  label: React.ReactNode;
  htmlFor?: string;
  hint?: React.ReactNode;
  error?: React.ReactNode;
  required?: boolean;
  renderState?: FormFieldRenderState;
  loadingContent?: React.ReactNode;
  emptyContent?: React.ReactNode;
  errorContent?: React.ReactNode;
}

export const FormField = React.forwardRef<HTMLDivElement, FormFieldProps>(
  (
    {
      className,
      orientation,
      state,
      label,
      htmlFor,
      hint,
      error,
      required,
      renderState = "content",
      children,
      loadingContent,
      emptyContent,
      errorContent,
      ...props
    },
    ref,
  ) => {
    const visualState = state ?? (renderState === "error" || error ? "error" : "default");

    const resolvedContent =
      renderState === "error"
        ? errorContent ?? children
        : renderState === "loading"
          ? loadingContent ?? children
          : renderState === "empty"
            ? emptyContent ?? children
            : children;

    const describedBy = htmlFor ? `${htmlFor}-status` : undefined;
    const statusNode =
      renderState === "error" || visualState === "error"
        ? (
            <StatusText id={describedBy} tone="error" state="error" size="sm">
              {errorContent ?? error}
            </StatusText>
          )
        : renderState === "loading"
          ? (
              <StatusText id={describedBy} tone="info" state="loading" size="sm">
                {loadingContent ?? hint}
              </StatusText>
            )
          : renderState === "empty"
            ? (
                <StatusText id={describedBy} tone="info" state="empty" emptyContent={emptyContent ?? hint} size="sm" />
              )
            : hint
              ? (
                  <StatusText id={describedBy} tone="info" size="sm">
                    {hint}
                  </StatusText>
                )
              : null;

    return (
      <div ref={ref} className={cn(formFieldVariants({ orientation, state: visualState }), className)} data-state={renderState} {...props}>
        <label
          htmlFor={htmlFor}
          className={cn(
            "text-sm font-medium leading-none",
            orientation === "horizontal" ? "pt-2" : "",
            visualState === "error" ? "text-destructive" : "text-foreground",
          )}
        >
          <span className="max-w-full break-words">
            {label}
            {required ? <span className="ml-1 text-destructive">*</span> : null}
          </span>
        </label>
        <div className="flex min-w-0 flex-col gap-2" aria-describedby={describedBy}>
          {resolvedContent}
          {statusNode}
        </div>
      </div>
    );
  },
);

FormField.displayName = "FormField";
