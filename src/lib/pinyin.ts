// ピンインの正規化と正誤判定。
// 声調記号形式（yī shēng）と数字形式（yi1 sheng1）を同一視できるよう、
// 「声調を剥がした字列」と「声調の並び」に分解して比較する。
// ※ 比較に必要なのは「記号→剥がす」方向のみで、配置規則を要する
//   「数字→記号」変換は不要。

const TONE_MAP: Record<string, [string, number]> = {
  "ā": ["a", 1], "á": ["a", 2], "ǎ": ["a", 3], "à": ["a", 4],
  "ē": ["e", 1], "é": ["e", 2], "ě": ["e", 3], "è": ["e", 4],
  "ī": ["i", 1], "í": ["i", 2], "ǐ": ["i", 3], "ì": ["i", 4],
  "ō": ["o", 1], "ó": ["o", 2], "ǒ": ["o", 3], "ò": ["o", 4],
  "ū": ["u", 1], "ú": ["u", 2], "ǔ": ["u", 3], "ù": ["u", 4],
  "ǖ": ["v", 1], "ǘ": ["v", 2], "ǚ": ["v", 3], "ǜ": ["v", 4],
};

export interface CanonPinyin {
  /** 声調・空白を除いた英字列（ü は v に正規化） */
  letters: string;
  /** 出現順の声調番号（軽声=5。1〜4のみが意味を持つ） */
  tones: number[];
}

/** ピンイン文字列を比較用の正規形に分解する */
export function canonPinyin(s: string): CanonPinyin {
  // 結合文字（NFD）を合成（NFC）してから処理し、ü / u: は v に統一
  const str = s.normalize("NFC").toLowerCase().replace(/u:/g, "v").replace(/ü/g, "v");
  let letters = "";
  const tones: number[] = [];
  for (const ch of str) {
    const mapped = TONE_MAP[ch];
    if (mapped) {
      letters += mapped[0];
      tones.push(mapped[1]);
    } else if (ch >= "0" && ch <= "5") {
      tones.push(ch === "0" ? 5 : Number(ch)); // 0/5 は軽声
    } else if (ch >= "a" && ch <= "z") {
      letters += ch;
    }
    // それ以外（空白・アポストロフィ・記号）は無視
  }
  return { letters, tones };
}

/** 軽声(5)を除いた声調列が一致するか */
function tonesEqual(a: number[], b: number[]): boolean {
  const x = a.filter((t) => t !== 5);
  const y = b.filter((t) => t !== 5);
  return x.length === y.length && x.every((v, i) => v === y[i]);
}

export type PinyinStatus = "correct" | "tone" | "incorrect";

export interface PinyinAnalysis {
  status: PinyinStatus;
  /** 差分表示・正解表示に使う accepted 形式（入力の書式に近いものを選ぶ） */
  target: string;
}

/** 入力の書式（数字 or 声調記号）に近い accepted 形式を選ぶ */
function pickTarget(input: string, accepted: string[], fallback: string): string {
  if (accepted.length === 0) return fallback;
  const inputNumbered = /[0-5]/.test(input);
  const numbered = accepted.filter((a) => /[0-5]/.test(a));
  const marked = accepted.filter((a) => !/[0-5]/.test(a));
  if (inputNumbered && numbered.length > 0) return numbered[0];
  if (!inputNumbered && marked.length > 0) return marked[0];
  return fallback;
}

/**
 * ユーザー入力を accepted 形式群と照合する。
 * - つづり+声調が一致 → "correct"
 * - つづりは一致するが声調が違う → "tone"
 * - つづりが違う → "incorrect"
 */
export function analyzePinyin(input: string, accepted: string[]): PinyinAnalysis {
  const ci = canonPinyin(input);
  const cands = accepted.map((a) => ({ raw: a, c: canonPinyin(a) }));

  const exact = cands.find(
    (x) => x.c.letters === ci.letters && tonesEqual(x.c.tones, ci.tones),
  );
  if (exact) return { status: "correct", target: pickTarget(input, accepted, exact.raw) };

  const lettersOnly = cands.find((x) => x.c.letters === ci.letters);
  if (lettersOnly) return { status: "tone", target: pickTarget(input, accepted, lettersOnly.raw) };

  return { status: "incorrect", target: pickTarget(input, accepted, accepted[0] ?? "") };
}
