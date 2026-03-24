CREATE TABLE "embedding_indexes" (
	"id" text PRIMARY KEY NOT NULL,
	"project_id" text NOT NULL,
	"property_name" text NOT NULL,
	"model_key" text NOT NULL,
	"dimensions" integer NOT NULL,
	"enabled" boolean DEFAULT true NOT NULL,
	"status" text DEFAULT 'pending' NOT NULL,
	"created_at" text NOT NULL,
	"updated_at" text NOT NULL
);
--> statement-breakpoint
ALTER TABLE "embedding_indexes" ADD CONSTRAINT "embedding_indexes_project_id_projects_id_fk" FOREIGN KEY ("project_id") REFERENCES "public"."projects"("id") ON DELETE cascade ON UPDATE no action;
