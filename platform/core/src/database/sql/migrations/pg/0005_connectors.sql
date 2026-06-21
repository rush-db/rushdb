CREATE TABLE IF NOT EXISTS "connectors" (
  "id" text PRIMARY KEY NOT NULL,
  "project_id" text NOT NULL,
  "name" text NOT NULL,
  "type" text NOT NULL,
  "config" text NOT NULL,
  "transform" text NOT NULL,
  "status" text DEFAULT 'paused' NOT NULL,
  "last_error" text,
  "lag_ms" integer,
  "stats" text,
  "created_by" text,
  "created_at" text NOT NULL,
  "updated_at" text NOT NULL
);

DO $$ BEGIN
 ALTER TABLE "connectors" ADD CONSTRAINT "connectors_project_id_projects_id_fk" FOREIGN KEY ("project_id") REFERENCES "public"."projects"("id") ON DELETE cascade ON UPDATE no action;
EXCEPTION
 WHEN duplicate_object THEN null;
END $$;

CREATE TABLE IF NOT EXISTS "connector_secrets" (
  "connector_id" text PRIMARY KEY NOT NULL,
  "provider" text DEFAULT 'local' NOT NULL,
  "secret_ref" text,
  "ciphertext" text,
  "created_at" text NOT NULL,
  "updated_at" text NOT NULL
);

DO $$ BEGIN
 ALTER TABLE "connector_secrets" ADD CONSTRAINT "connector_secrets_connector_id_connectors_id_fk" FOREIGN KEY ("connector_id") REFERENCES "public"."connectors"("id") ON DELETE cascade ON UPDATE no action;
EXCEPTION
 WHEN duplicate_object THEN null;
END $$;

CREATE TABLE IF NOT EXISTS "connector_offsets" (
  "connector_id" text NOT NULL,
  "partition" text NOT NULL,
  "position" text NOT NULL,
  "updated_at" text NOT NULL,
  CONSTRAINT "connector_offsets_connector_id_partition_pk" PRIMARY KEY("connector_id","partition")
);

DO $$ BEGIN
 ALTER TABLE "connector_offsets" ADD CONSTRAINT "connector_offsets_connector_id_connectors_id_fk" FOREIGN KEY ("connector_id") REFERENCES "public"."connectors"("id") ON DELETE cascade ON UPDATE no action;
EXCEPTION
 WHEN duplicate_object THEN null;
END $$;

CREATE TABLE IF NOT EXISTS "connector_events" (
  "id" text PRIMARY KEY NOT NULL,
  "connector_id" text NOT NULL,
  "project_id" text NOT NULL,
  "level" text DEFAULT 'info' NOT NULL,
  "type" text NOT NULL,
  "message" text NOT NULL,
  "metadata" text,
  "created_at" text NOT NULL
);

DO $$ BEGIN
 ALTER TABLE "connector_events" ADD CONSTRAINT "connector_events_connector_id_connectors_id_fk" FOREIGN KEY ("connector_id") REFERENCES "public"."connectors"("id") ON DELETE cascade ON UPDATE no action;
EXCEPTION
 WHEN duplicate_object THEN null;
END $$;

DO $$ BEGIN
 ALTER TABLE "connector_events" ADD CONSTRAINT "connector_events_project_id_projects_id_fk" FOREIGN KEY ("project_id") REFERENCES "public"."projects"("id") ON DELETE cascade ON UPDATE no action;
EXCEPTION
 WHEN duplicate_object THEN null;
END $$;

CREATE TABLE IF NOT EXISTS "connector_leases" (
  "connector_id" text PRIMARY KEY NOT NULL,
  "worker_id" text NOT NULL,
  "lease_until" text NOT NULL,
  "heartbeat_at" text NOT NULL,
  "created_at" text NOT NULL,
  "updated_at" text NOT NULL
);

DO $$ BEGIN
 ALTER TABLE "connector_leases" ADD CONSTRAINT "connector_leases_connector_id_connectors_id_fk" FOREIGN KEY ("connector_id") REFERENCES "public"."connectors"("id") ON DELETE cascade ON UPDATE no action;
EXCEPTION
 WHEN duplicate_object THEN null;
END $$;
