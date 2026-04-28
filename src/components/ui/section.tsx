import * as React from "react";
import { cva } from "class-variance-authority";
import { cn } from "@/lib/utils";

type SectionState = "content" | "loading" | "empty" | "error";

const sectionVariants = cva("rounded-lg border bg-card text-card-foreground p-4", {
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

export interface SectionProps extends React.HTMLAttributes<HTMLElement> {
  as?: "section" | "div";
  state?: SectionState;
  loadingContent?: React.ReactNode;
  emptyContent?: React.ReactNode;
  errorContent?: React.ReactNode;
}

function Section({
  className,
  as = "section",
  state = "content",
  children,
  loadingContent,
  emptyContent,
  errorContent,
  ...props
}: SectionProps) {
  const Comp = as;
  const resolvedContent =
    state === "error"
      ? errorContent ?? children
      : state === "loading"
        ? loadingContent ?? children
        : state === "empty"
          ? emptyContent ?? children
          : children;

  return (
    <Comp className={cn(sectionVariants({ state }), "flex flex-col gap-6", className)} data-state={state} {...props}>
      {resolvedContent}
    </Comp>
  );
}

export interface SectionHeaderProps extends Omit<React.HTMLAttributes<HTMLDivElement>, "title"> {
  title?: React.ReactNode;
  description?: React.ReactNode;
  titleAs?: "h2" | "h3" | "h4";
}

const SectionHeader = React.forwardRef<HTMLDivElement, SectionHeaderProps>(
  ({ className, title, description, titleAs = "h2", children, ...props }, ref) => {
    const Title = titleAs;
    return (
      <div ref={ref} className={cn("flex flex-col gap-1.5", className)} {...props}>
        {title && <Title className="truncate text-base font-semibold leading-none tracking-tight">{title}</Title>}
        {description && <p className="text-sm text-muted-foreground">{description}</p>}
        {children}
      </div>
    );
  },
);
SectionHeader.displayName = "SectionHeader";

const SectionBody = React.forwardRef<HTMLDivElement, React.HTMLAttributes<HTMLDivElement>>(
  ({ className, ...props }, ref) => <div ref={ref} className={cn("flex flex-col gap-4", className)} {...props} />,
);
SectionBody.displayName = "SectionBody";

export { Section, SectionBody, SectionHeader };
