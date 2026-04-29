import * as React from "react";
import { open, save } from "@tauri-apps/plugin-dialog";
import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "@/lib/utils";
import { Button } from "./button";
import { StatusText } from "./status-text";

type PathPickerState = "content" | "loading" | "empty" | "error";
type PathPickerMode = "directory" | "file-open" | "file-save";

export interface PathPickerFilter {
  name: string;
  extensions: string[];
}

const pathPickerRootVariants = cva("flex w-full flex-col gap-2", {
  variants: {
    mode: {
      directory: "",
      "file-open": "",
      "file-save": "",
    },
    state: {
      content: "",
      loading: "opacity-90",
      empty: "",
      error: "",
    },
  },
  defaultVariants: {
    mode: "directory",
    state: "content",
  },
});

const pathValueVariants = cva("inline-flex h-8 flex-1 items-center rounded-md border bg-background px-3", {
  variants: {
    state: {
      content: "border-input text-foreground",
      loading: "border-input text-muted-foreground",
      empty: "border-input text-muted-foreground",
      error: "border-destructive text-destructive",
    },
  },
  defaultVariants: {
    state: "content",
  },
});

export interface PathPickerProps
  extends Omit<React.HTMLAttributes<HTMLDivElement>, "onChange">,
    VariantProps<typeof pathPickerRootVariants> {
  value: string;
  onChange: (path: string) => void;
  mode?: PathPickerMode;
  label?: React.ReactNode;
  placeholder?: React.ReactNode;
  defaultPath?: string;
  filters?: PathPickerFilter[];
  buttonLabel?: React.ReactNode;
  disabled?: boolean;
  state?: PathPickerState;
  loadingMessage?: React.ReactNode;
  emptyMessage?: React.ReactNode;
  errorMessage?: React.ReactNode;
  onPickError?: (message: string) => void;
}

export const PathPicker = React.forwardRef<HTMLDivElement, PathPickerProps>(
  (
    {
      className,
      value,
      onChange,
      mode = "directory",
      label,
      placeholder = "No path selected",
      defaultPath,
      filters,
      buttonLabel = "Select Path",
      disabled = false,
      state,
      loadingMessage = "Opening picker...",
      emptyMessage,
      errorMessage,
      onPickError,
      ...props
    },
    ref,
  ) => {
    const [isPicking, setIsPicking] = React.useState(false);
    const [runtimeError, setRuntimeError] = React.useState<string | null>(null);

    const derivedState: PathPickerState = state ?? (value.trim().length > 0 ? "content" : "empty");
    const visualState: PathPickerState =
      runtimeError != null
        ? "error"
        : isPicking
          ? "loading"
          : derivedState;
    const isDisabled = disabled || visualState === "loading";

    const pickPath = React.useCallback(async () => {
      if (isPicking || disabled) {
        return;
      }

      setRuntimeError(null);
      setIsPicking(true);
      try {
        const selected =
          mode === "file-save"
            ? await save({ defaultPath, filters })
            : await open({
                directory: mode === "directory",
                multiple: false,
                defaultPath,
                filters: mode === "file-open" ? filters : undefined,
              });

        if (!selected || Array.isArray(selected)) {
          return;
        }

        onChange(selected);
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        setRuntimeError(message);
        onPickError?.(message);
      } finally {
        setIsPicking(false);
      }
    }, [defaultPath, disabled, filters, isPicking, mode, onChange, onPickError]);

    const visiblePath = value.trim().length > 0 ? value : placeholder;
    const showStatus = visualState === "loading" || visualState === "error" || visualState === "empty";

    return (
      <div ref={ref} className={cn(pathPickerRootVariants({ mode, state: visualState }), className)} data-state={visualState} {...props}>
        {label ? <span className="text-sm font-medium text-foreground">{label}</span> : null}
        <div className="flex items-center gap-2">
          <div className={cn(pathValueVariants({ state: visualState }))} aria-live="polite">
            <span className="block w-full truncate text-sm leading-none">{visiblePath}</span>
          </div>
          <Button
            type="button"
            variant="secondary"
            size="md"
            state={visualState === "loading" ? "loading" : "content"}
            loadingContent={buttonLabel}
            onClick={() => void pickPath()}
            disabled={isDisabled}
            aria-label={typeof buttonLabel === "string" ? buttonLabel : "Select path"}
          >
            {buttonLabel}
          </Button>
        </div>
        {showStatus ? (
          <StatusText tone={visualState === "error" ? "error" : "info"} size="sm" state={visualState === "error" ? "error" : "content"}>
            {visualState === "loading"
              ? loadingMessage
              : visualState === "error"
                ? errorMessage ?? runtimeError
                : emptyMessage ?? null}
          </StatusText>
        ) : null}
      </div>
    );
  },
);

PathPicker.displayName = "PathPicker";
