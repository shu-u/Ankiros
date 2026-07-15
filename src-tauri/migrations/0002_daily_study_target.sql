-- 1日の学習量の目安（新規カードの自動調整用）。
-- 意図: 復習が多い日に、新規＋復習の合計がこの枚数を超えないよう「新規のみ」を自動で減らす。
--       復習は絞らない（SRS 上、復習を遅延させないため常に daily_review_limit まで出題する）。
-- NULL = 無効（従来どおり新規は daily_new_limit のみで頭打ち）。既存デッキは NULL となり挙動不変。
ALTER TABLE decks ADD COLUMN daily_study_target INTEGER;
