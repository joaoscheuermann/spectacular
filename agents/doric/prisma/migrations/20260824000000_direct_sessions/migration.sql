CREATE TYPE "ModelRole" AS ENUM ('EXECUTION');

CREATE TYPE "SessionState" AS ENUM (
  'QUEUED',
  'READY',
  'RUNNING',
  'CANCELLING',
  'FAILED',
  'CANCELLED'
);

CREATE TABLE "doric_configuration" (
  "id" INTEGER NOT NULL DEFAULT 1,
  "revision" INTEGER NOT NULL DEFAULT 1,
  "generation" UUID NOT NULL,
  "max_turns" INTEGER NOT NULL,
  "updated_at" TIMESTAMPTZ(3) NOT NULL,
  CONSTRAINT "doric_configuration_pkey" PRIMARY KEY ("id"),
  CONSTRAINT "doric_configuration_singleton" CHECK ("id" = 1)
);

CREATE TABLE "provider_configuration" (
  "configuration_id" INTEGER NOT NULL,
  "id" TEXT NOT NULL,
  "base_url" TEXT NOT NULL,
  "api_key_env" TEXT NOT NULL,
  CONSTRAINT "provider_configuration_pkey" PRIMARY KEY ("configuration_id", "id")
);

CREATE TABLE "model_configuration" (
  "configuration_id" INTEGER NOT NULL,
  "role" "ModelRole" NOT NULL,
  "provider_id" TEXT NOT NULL,
  "model" TEXT NOT NULL,
  "effort" TEXT NOT NULL,
  CONSTRAINT "model_configuration_pkey" PRIMARY KEY ("configuration_id", "role")
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

ALTER TABLE "provider_configuration"
  ADD CONSTRAINT "provider_configuration_configuration_id_fkey"
  FOREIGN KEY ("configuration_id") REFERENCES "doric_configuration"("id")
  ON DELETE CASCADE ON UPDATE CASCADE;

ALTER TABLE "model_configuration"
  ADD CONSTRAINT "model_configuration_configuration_id_fkey"
  FOREIGN KEY ("configuration_id") REFERENCES "doric_configuration"("id")
  ON DELETE CASCADE ON UPDATE CASCADE;

ALTER TABLE "model_configuration"
  ADD CONSTRAINT "model_configuration_configuration_id_provider_id_fkey"
  FOREIGN KEY ("configuration_id", "provider_id")
  REFERENCES "provider_configuration"("configuration_id", "id")
  ON DELETE RESTRICT ON UPDATE CASCADE;

ALTER TABLE "session_event"
  ADD CONSTRAINT "session_event_session_id_fkey"
  FOREIGN KEY ("session_id") REFERENCES "session"("id")
  ON DELETE CASCADE ON UPDATE CASCADE;

INSERT INTO "doric_configuration" (
  "id",
  "revision",
  "generation",
  "max_turns",
  "updated_at"
) VALUES (
  1,
  1,
  '00000000-0000-4000-8000-000000000001',
  32,
  CURRENT_TIMESTAMP
);

INSERT INTO "provider_configuration" (
  "configuration_id", "id", "base_url", "api_key_env"
) VALUES (
  1, 'openrouter', 'https://openrouter.ai/api/v1', 'OPENROUTER_API_KEY'
);

INSERT INTO "model_configuration" (
  "configuration_id", "role", "provider_id", "model", "effort"
) VALUES (
  1,
  'EXECUTION',
  'openrouter',
  'deepseek/deepseek-v4-flash-0731',
  'low'
);
