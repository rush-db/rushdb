CREATE TABLE "oauth_refresh_tokens" (
	"id" text PRIMARY KEY NOT NULL,
	"consent_id" text NOT NULL,
	"client_id" text NOT NULL,
	"user_id" text NOT NULL,
	"project_id" text NOT NULL,
	"scope" text NOT NULL,
	"created_at" text NOT NULL,
	"expires_at" text NOT NULL
);
--> statement-breakpoint
ALTER TABLE "oauth_refresh_tokens" ADD CONSTRAINT "oauth_refresh_tokens_consent_id_oauth_consents_id_fk" FOREIGN KEY ("consent_id") REFERENCES "public"."oauth_consents"("id") ON DELETE cascade ON UPDATE no action;
