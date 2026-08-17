CREATE TABLE "connector_definitions" (
	"id" text PRIMARY KEY NOT NULL,
	"descriptor" text NOT NULL,
	"version" text DEFAULT '1' NOT NULL,
	"registered_by" text,
	"updated_at" text NOT NULL
);
