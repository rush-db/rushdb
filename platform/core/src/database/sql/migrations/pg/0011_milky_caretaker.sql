CREATE TABLE "connector_runs" (
	"id" text PRIMARY KEY NOT NULL,
	"connector_id" text NOT NULL,
	"project_id" text NOT NULL,
	"worker_id" text NOT NULL,
	"trigger" text DEFAULT 'user' NOT NULL,
	"status" text DEFAULT 'running' NOT NULL,
	"phase" text DEFAULT 'starting' NOT NULL,
	"records_read" integer DEFAULT 0 NOT NULL,
	"records_written" integer DEFAULT 0 NOT NULL,
	"records_rejected" integer DEFAULT 0 NOT NULL,
	"error_message" text,
	"started_at" text NOT NULL,
	"completed_at" text,
	"heartbeat_at" text NOT NULL
);
--> statement-breakpoint
ALTER TABLE "connector_runs" ADD CONSTRAINT "connector_runs_connector_id_connectors_id_fk" FOREIGN KEY ("connector_id") REFERENCES "public"."connectors"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "connector_runs" ADD CONSTRAINT "connector_runs_project_id_projects_id_fk" FOREIGN KEY ("project_id") REFERENCES "public"."projects"("id") ON DELETE cascade ON UPDATE no action;