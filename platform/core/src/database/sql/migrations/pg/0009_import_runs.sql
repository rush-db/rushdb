CREATE TABLE IF NOT EXISTS "import_runs" (
	"id" text PRIMARY KEY NOT NULL,
	"project_id" text NOT NULL,
	"workspace_id" text,
	"created_by_type" text DEFAULT 'user' NOT NULL,
	"created_by_id" text,
	"name" text,
	"idempotency_key_hash" text,
	"status" text DEFAULT 'draft' NOT NULL,
	"failure_policy" text DEFAULT 'continue' NOT NULL,
	"manifest_version" integer DEFAULT 0 NOT NULL,
	"total_files" integer DEFAULT 0 NOT NULL,
	"total_bytes" bigint DEFAULT 0 NOT NULL,
	"uploaded_bytes" bigint DEFAULT 0 NOT NULL,
	"parsed_units" integer DEFAULT 0 NOT NULL,
	"records_committed" integer DEFAULT 0 NOT NULL,
	"relationships_committed" integer DEFAULT 0 NOT NULL,
	"skipped_units" integer DEFAULT 0 NOT NULL,
	"failed_files" integer DEFAULT 0 NOT NULL,
	"cancel_requested_at" text,
	"started_at" text,
	"finalized_at" text,
	"retention_until" text,
	"created_at" text NOT NULL,
	"updated_at" text NOT NULL
);
--> statement-breakpoint
ALTER TABLE "import_runs" ADD CONSTRAINT "import_runs_project_id_projects_id_fk" FOREIGN KEY ("project_id") REFERENCES "public"."projects"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
CREATE TABLE IF NOT EXISTS "import_run_files" (
	"id" text PRIMARY KEY NOT NULL,
	"run_id" text NOT NULL,
	"project_id" text NOT NULL,
	"workspace_id" text,
	"ordinal" integer NOT NULL,
	"client_file_id" text NOT NULL,
	"file_name" text NOT NULL,
	"declared_size_bytes" bigint DEFAULT 0 NOT NULL,
	"format" text NOT NULL,
	"json_shape" text,
	"role" text DEFAULT 'records' NOT NULL,
	"root_label" text,
	"link_spec" text,
	"parse_options" text,
	"import_options" text,
	"source_generation" integer DEFAULT 1 NOT NULL,
	"storage_provider" text DEFAULT 'memory' NOT NULL,
	"storage_key" text,
	"storage_upload_id" text,
	"object_size_bytes" bigint,
	"checksum_algorithm" text,
	"checksum_value" text,
	"status" text DEFAULT 'awaiting_upload' NOT NULL,
	"stage" text DEFAULT 'upload' NOT NULL,
	"processed_bytes" bigint DEFAULT 0 NOT NULL,
	"parsed_units" integer DEFAULT 0 NOT NULL,
	"committed_units" integer DEFAULT 0 NOT NULL,
	"records_committed" integer DEFAULT 0 NOT NULL,
	"relationships_committed" integer DEFAULT 0 NOT NULL,
	"links_resolved" integer DEFAULT 0 NOT NULL,
	"links_unresolved" integer DEFAULT 0 NOT NULL,
	"skipped_units" integer DEFAULT 0 NOT NULL,
	"current_batch" integer DEFAULT 0 NOT NULL,
	"checkpoint" text,
	"attempt_count" integer DEFAULT 0 NOT NULL,
	"max_attempts" integer DEFAULT 3 NOT NULL,
	"not_before" text,
	"lease_owner" text,
	"lease_generation" integer DEFAULT 0 NOT NULL,
	"lease_until" text,
	"heartbeat_at" text,
	"cancel_requested_at" text,
	"last_error_code" text,
	"last_error_message" text,
	"started_at" text,
	"finished_at" text,
	"created_at" text NOT NULL,
	"updated_at" text NOT NULL
);
--> statement-breakpoint
ALTER TABLE "import_run_files" ADD CONSTRAINT "import_run_files_run_id_import_runs_id_fk" FOREIGN KEY ("run_id") REFERENCES "public"."import_runs"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "import_run_files" ADD CONSTRAINT "import_run_files_project_id_projects_id_fk" FOREIGN KEY ("project_id") REFERENCES "public"."projects"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
CREATE UNIQUE INDEX IF NOT EXISTS "import_file_client_id_uniq" ON "import_run_files" ("run_id","client_file_id");--> statement-breakpoint
CREATE UNIQUE INDEX IF NOT EXISTS "import_file_ordinal_uniq" ON "import_run_files" ("run_id","ordinal");--> statement-breakpoint
CREATE INDEX IF NOT EXISTS "import_file_claim_idx" ON "import_run_files" ("status","lease_until","not_before");--> statement-breakpoint
CREATE INDEX IF NOT EXISTS "import_file_role_idx" ON "import_run_files" ("run_id","role");--> statement-breakpoint
CREATE TABLE IF NOT EXISTS "import_run_events" (
	"id" text PRIMARY KEY NOT NULL,
	"run_id" text NOT NULL,
	"file_id" text,
	"project_id" text NOT NULL,
	"type" text NOT NULL,
	"from_status" text,
	"to_status" text,
	"code" text,
	"message" text,
	"attempt" integer,
	"lease_generation" integer,
	"metadata" text,
	"created_at" text NOT NULL
);
--> statement-breakpoint
ALTER TABLE "import_run_events" ADD CONSTRAINT "import_run_events_run_id_import_runs_id_fk" FOREIGN KEY ("run_id") REFERENCES "public"."import_runs"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "import_run_events" ADD CONSTRAINT "import_run_events_project_id_projects_id_fk" FOREIGN KEY ("project_id") REFERENCES "public"."projects"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
CREATE INDEX IF NOT EXISTS "import_event_run_idx" ON "import_run_events" ("run_id","created_at");--> statement-breakpoint
CREATE TABLE IF NOT EXISTS "import_error_samples" (
	"id" text PRIMARY KEY NOT NULL,
	"run_id" text NOT NULL,
	"file_id" text NOT NULL,
	"project_id" text NOT NULL,
	"source_unit" integer,
	"line_number" integer,
	"column_number" integer,
	"code" text NOT NULL,
	"message" text,
	"created_at" text NOT NULL
);
--> statement-breakpoint
ALTER TABLE "import_error_samples" ADD CONSTRAINT "import_error_samples_run_id_import_runs_id_fk" FOREIGN KEY ("run_id") REFERENCES "public"."import_runs"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "import_error_samples" ADD CONSTRAINT "import_error_samples_file_id_import_run_files_id_fk" FOREIGN KEY ("file_id") REFERENCES "public"."import_run_files"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "import_error_samples" ADD CONSTRAINT "import_error_samples_project_id_projects_id_fk" FOREIGN KEY ("project_id") REFERENCES "public"."projects"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
CREATE INDEX IF NOT EXISTS "import_error_sample_file_idx" ON "import_error_samples" ("file_id","created_at");--> statement-breakpoint
CREATE UNIQUE INDEX IF NOT EXISTS "import_run_idempotency_uniq" ON "import_runs" ("project_id","idempotency_key_hash");
