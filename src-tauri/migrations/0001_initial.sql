-- 単語帳アプリ 初期スキーマ (spec §3.3)
-- 注意: PRAGMA foreign_keys = ON は接続オプション側 (SqliteConnectOptions::foreign_keys(true)) で
-- 全プール接続に対して有効化する。

CREATE TABLE decks (
  id                      TEXT PRIMARY KEY,
  name                    TEXT NOT NULL,
  description             TEXT,
  language                TEXT DEFAULT 'zh',
  test_modes              TEXT NOT NULL,        -- JSON配列 ["recognition","pronunciation"]
  daily_new_limit         INTEGER DEFAULT 20,
  daily_review_limit      INTEGER DEFAULT 100,
  fsrs_target_retention   REAL DEFAULT 0.90,
  fsrs_max_interval_days  INTEGER DEFAULT 365,
  created_at              TEXT NOT NULL,
  updated_at              TEXT NOT NULL
);

CREATE TABLE cards (
  id                  TEXT NOT NULL,
  deck_id             TEXT NOT NULL,
  hanzi               TEXT NOT NULL,
  pinyin_accepted     TEXT NOT NULL,   -- JSON配列
  meaning             TEXT NOT NULL,
  example_sentences   TEXT,            -- JSON配列
  synonyms            TEXT,            -- JSON配列
  antonyms            TEXT,            -- JSON配列
  tags                TEXT,            -- JSON配列
  ai_notes            TEXT,            -- JSONの"notes"フィールドからマッピング
  user_notes          TEXT DEFAULT '',
  audio_path          TEXT,            -- 将来の音声機能用（v1では常にNULL）
  created_at          TEXT NOT NULL,
  updated_at          TEXT NOT NULL,
  PRIMARY KEY (id, deck_id),
  FOREIGN KEY (deck_id) REFERENCES decks(id) ON DELETE CASCADE
);

CREATE TABLE srs_records (
  card_id         TEXT NOT NULL,
  deck_id         TEXT NOT NULL,
  mode            TEXT NOT NULL,   -- 'recognition'|'pronunciation'|'production'
  due_date        TEXT NOT NULL,   -- ISO 8601 UTC datetime
  stability       REAL,
  difficulty      REAL,
  state           TEXT DEFAULT 'new',  -- 'new'|'learning'|'review'|'relearning'
  reps            INTEGER DEFAULT 0,
  lapses          INTEGER DEFAULT 0,
  last_review     TEXT,
  scheduled_days  INTEGER DEFAULT 0,
  elapsed_days    INTEGER DEFAULT 0,
  PRIMARY KEY (card_id, deck_id, mode),
  FOREIGN KEY (card_id, deck_id) REFERENCES cards(id, deck_id) ON DELETE CASCADE
);

CREATE TABLE review_logs (
  id          TEXT PRIMARY KEY,   -- UUID
  card_id     TEXT NOT NULL,
  deck_id     TEXT NOT NULL,
  mode        TEXT NOT NULL,
  rating      TEXT NOT NULL,      -- 'again'|'hard'|'good'|'easy'
  reviewed_at TEXT NOT NULL,      -- ISO 8601 UTC datetime
  FOREIGN KEY (card_id, deck_id) REFERENCES cards(id, deck_id) ON DELETE CASCADE
);

CREATE TABLE app_state (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

-- インデックス（セッションキュー構築・統計集計の高速化）
CREATE INDEX idx_cards_deck ON cards(deck_id);
CREATE INDEX idx_srs_due ON srs_records(deck_id, mode, due_date);
CREATE INDEX idx_review_logs_reviewed ON review_logs(reviewed_at);
CREATE INDEX idx_review_logs_card ON review_logs(card_id, deck_id);

-- 既定のアプリ状態
INSERT INTO app_state (key, value) VALUES ('theme', 'light');
INSERT INTO app_state (key, value) VALUES ('window_width', '1200');
INSERT INTO app_state (key, value) VALUES ('window_height', '800');
