import { diffChars } from "diff";

/** ピンイン比較の正規化 (spec §6.4): 前後空白除去・大文字小文字無視。
 * 声調記号↔数字の変換は行わない。 */
function normalize(s: string): string {
  return s.trim().toLowerCase();
}

interface Props {
  user: string;
  correct: string;
}

/**
 * pronunciation / production モードの文字単位差分ハイライト (spec §6.4)。
 * ユーザー入力行と正解行を上下に並べて表示する。
 */
export function PinyinDiff({ user, correct }: Props) {
  const parts = diffChars(normalize(user), normalize(correct));

  return (
    <div className="space-y-1 font-mono text-base">
      <div className="flex flex-wrap items-baseline gap-x-1">
        <span className="mr-2 shrink-0 font-sans text-sm text-muted-foreground">
          あなたの入力：
        </span>
        <span>
          {user.trim() === "" ? (
            <span className="text-muted-foreground">（空）</span>
          ) : (
            parts.map((p, i) =>
              p.added ? null : (
                <span key={i} className={p.removed ? "diff-removed" : ""}>
                  {p.value}
                </span>
              ),
            )
          )}
        </span>
      </div>
      <div className="flex flex-wrap items-baseline gap-x-1">
        <span className="mr-2 shrink-0 font-sans text-sm text-muted-foreground">
          正解：
        </span>
        <span>
          {parts.map((p, i) =>
            p.removed ? null : (
              <span key={i} className={p.added ? "diff-added" : ""}>
                {p.value}
              </span>
            ),
          )}
        </span>
      </div>
    </div>
  );
}
