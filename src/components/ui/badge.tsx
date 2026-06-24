import * as React from "react";
import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "@/lib/utils";

const badgeVariants = cva(
  "inline-flex items-center rounded-full border px-2.5 py-0.5 text-xs font-semibold transition-colors",
  {
    variants: {
      variant: {
        default: "border-transparent bg-primary text-primary-foreground",
        secondary: "border-transparent bg-secondary text-secondary-foreground",
        destructive: "border-transparent bg-destructive text-destructive-foreground",
        outline: "text-foreground",
        new: "border-transparent bg-slate-200 text-slate-700 dark:bg-slate-700 dark:text-slate-200",
        learning: "border-transparent bg-amber-200 text-amber-900 dark:bg-amber-700 dark:text-amber-100",
        review: "border-transparent bg-green-200 text-green-900 dark:bg-green-800 dark:text-green-100",
        relearning: "border-transparent bg-red-200 text-red-900 dark:bg-red-800 dark:text-red-100",
      },
    },
    defaultVariants: {
      variant: "default",
    },
  },
);

export interface BadgeProps
  extends React.HTMLAttributes<HTMLDivElement>,
    VariantProps<typeof badgeVariants> {}

function Badge({ className, variant, ...props }: BadgeProps) {
  return <div className={cn(badgeVariants({ variant }), className)} {...props} />;
}

export { Badge, badgeVariants };
