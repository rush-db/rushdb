CREATE TABLE "relationship_analysis_queue" (
	"project_id" text PRIMARY KEY NOT NULL,
	"requested_at" text NOT NULL,
	"not_before" text NOT NULL,
	"status" text DEFAULT 'pending' NOT NULL,
	"last_run_at" text,
	"last_error" text,
	"created_at" text NOT NULL,
	"updated_at" text NOT NULL
);
--> statement-breakpoint
CREATE TABLE "relationship_patterns" (
	"id" text PRIMARY KEY NOT NULL,
	"project_id" text NOT NULL,
	"source_label" text NOT NULL,
	"source_key" text,
	"source_where" text,
	"target_label" text NOT NULL,
	"target_key" text,
	"target_where" text,
	"direction" text DEFAULT 'out' NOT NULL,
	"type" text NOT NULL,
	"confidence" integer DEFAULT 0 NOT NULL,
	"status" text DEFAULT 'suggested' NOT NULL,
	"origin" text DEFAULT 'llm' NOT NULL,
	"signature_hash" text NOT NULL,
	"rationale" text,
	"sample_match_count" integer,
	"last_applied_at" text,
	"last_analyzed_at" text,
	"last_error" text,
	"created_at" text NOT NULL,
	"updated_at" text NOT NULL
);
--> statement-breakpoint
ALTER TABLE "relationship_analysis_queue" ADD CONSTRAINT "relationship_analysis_queue_project_id_projects_id_fk" FOREIGN KEY ("project_id") REFERENCES "public"."projects"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "relationship_patterns" ADD CONSTRAINT "relationship_patterns_project_id_projects_id_fk" FOREIGN KEY ("project_id") REFERENCES "public"."projects"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
CREATE UNIQUE INDEX "rel_pattern_signature_uniq" ON "relationship_patterns" USING btree ("project_id","signature_hash");
