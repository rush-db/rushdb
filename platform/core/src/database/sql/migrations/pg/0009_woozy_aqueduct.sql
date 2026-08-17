CREATE TABLE "connector_commands" (
	"id" text PRIMARY KEY NOT NULL,
	"connector_id" text NOT NULL,
	"project_id" text NOT NULL,
	"type" text NOT NULL,
	"status" text DEFAULT 'pending' NOT NULL,
	"payload" text,
	"result" text,
	"error_message" text,
	"requested_by" text,
	"claimed_by" text,
	"created_at" text NOT NULL,
	"claimed_at" text,
	"completed_at" text
);
--> statement-breakpoint
ALTER TABLE "connector_commands" ADD CONSTRAINT "connector_commands_connector_id_connectors_id_fk" FOREIGN KEY ("connector_id") REFERENCES "public"."connectors"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "connector_commands" ADD CONSTRAINT "connector_commands_project_id_projects_id_fk" FOREIGN KEY ("project_id") REFERENCES "public"."projects"("id") ON DELETE cascade ON UPDATE no action;