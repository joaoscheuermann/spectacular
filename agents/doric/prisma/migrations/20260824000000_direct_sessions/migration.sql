DROP TABLE "mosaic_event";
DROP TABLE "mosaic_session";
DROP TYPE "MosaicSessionState";

CREATE TYPE "SessionState" AS ENUM (
  'QUEUED',
  'READY',
  'RUNNING',
  'CANCELLING',
  'FAILED',
  'CANCELLED'
);

CREATE TABLE "session" (
  "id" UUID NOT NULL,
  "state" "SessionState" NOT NULL DEFAULT 'QUEUED',
  "config_revision" INTEGER NOT NULL,
  "config_snapshot" JSONB NOT NULL,
  "messages" JSONB NOT NULL DEFAULT '[]'::JSONB,
  "error_code" TEXT,
  "last_sequence" INTEGER NOT NULL DEFAULT 0,
  "created_at" TIMESTAMPTZ(3) NOT NULL DEFAULT CURRENT_TIMESTAMP,
  "updated_at" TIMESTAMPTZ(3) NOT NULL,
  "started_at" TIMESTAMPTZ(3),
  "finished_at" TIMESTAMPTZ(3),
  CONSTRAINT "session_pkey" PRIMARY KEY ("id")
);

CREATE TABLE "session_event" (
  "session_id" UUID NOT NULL,
  "prompt_id" UUID NOT NULL,
  "sequence" INTEGER NOT NULL,
  "type" TEXT NOT NULL,
  "event" JSONB NOT NULL,
  "created_at" TIMESTAMPTZ(3) NOT NULL DEFAULT CURRENT_TIMESTAMP,
  CONSTRAINT "session_event_pkey" PRIMARY KEY ("session_id", "sequence")
);

CREATE INDEX "session_created_at_id_idx" ON "session"("created_at", "id");

ALTER TABLE "session_event"
  ADD CONSTRAINT "session_event_session_id_fkey"
  FOREIGN KEY ("session_id") REFERENCES "session"("id")
  ON DELETE CASCADE ON UPDATE CASCADE;
