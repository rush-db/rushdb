CREATE TABLE "saved_queries" (
	"id" text PRIMARY KEY NOT NULL,
	"project_id" text NOT NULL,
	"name" text NOT NULL,
	"search_mode" text DEFAULT 'manual' NOT NULL,
	"prompt" text,
	"search_query" text NOT NULL,
	"semantic_index_id" text,
	"created_by" text,
	"created_at" text NOT NULL,
	"updated_at" text NOT NULL
);
--> statement-breakpoint
ALTER TABLE "saved_queries" ADD CONSTRAINT "saved_queries_project_id_projects_id_fk" FOREIGN KEY ("project_id") REFERENCES "public"."projects"("id") ON DELETE cascade ON UPDATE no action;