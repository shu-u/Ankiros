import { Badge } from "@/components/ui/badge";

export function Loading({ label = "読み込み中…" }: { label?: string }) {
  return (
    <div className="flex items-center justify-center p-10 text-sm text-muted-foreground">
      {label}
    </div>
  );
}

export function ErrorBox({ message }: { message: string }) {
  return (
    <div className="rounded-md border border-destructive/50 bg-destructive/10 p-4 text-sm text-destructive">
      {message}
    </div>
  );
}

const STATE_LABEL: Record<string, string> = {
  new: "New",
  learning: "Learning",
  review: "Review",
  relearning: "Relearning",
};

const STATE_VARIANT: Record<string, "new" | "learning" | "review" | "relearning"> = {
  new: "new",
  learning: "learning",
  review: "review",
  relearning: "relearning",
};

export function StateBadge({ state }: { state: string }) {
  return (
    <Badge variant={STATE_VARIANT[state] ?? "new"}>
      {STATE_LABEL[state] ?? state}
    </Badge>
  );
}

export const MODE_LABEL: Record<string, string> = {
  recognition: "認識",
  pronunciation: "発音",
  production: "産出",
  listening: "リスニング",
};

export function modeLabel(mode: string): string {
  return MODE_LABEL[mode] ?? mode;
}
