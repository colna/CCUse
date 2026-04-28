-- T1.0.4.04 — Add quota, rate-limit, and cost fields to providers.
--
-- All nullable so existing rows need no backfill.

ALTER TABLE providers ADD COLUMN monthly_quota    INTEGER DEFAULT NULL;
ALTER TABLE providers ADD COLUMN rate_limit_rpm   INTEGER DEFAULT NULL;
ALTER TABLE providers ADD COLUMN cost_per_1k_tokens REAL DEFAULT NULL;
