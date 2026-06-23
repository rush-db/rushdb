CREATE TABLE "workspace_identity_providers" (
	"id" text PRIMARY KEY NOT NULL,
	"workspace_id" text NOT NULL,
	"type" text NOT NULL,
	"enabled" boolean DEFAULT false NOT NULL,
	"enforced" boolean DEFAULT false NOT NULL,
	"domains" text DEFAULT '[]' NOT NULL,
	"default_role" text DEFAULT 'developer' NOT NULL,
	"group_mappings" text,
	"saml_entity_id" text,
	"saml_sso_url" text,
	"saml_certificate" text,
	"oidc_issuer" text,
	"oidc_client_id" text,
	"oidc_client_secret_encrypted" text,
	"created_at" text NOT NULL,
	"updated_at" text NOT NULL
);
--> statement-breakpoint
ALTER TABLE "users" ADD COLUMN "saml_auth" text;--> statement-breakpoint
ALTER TABLE "users" ADD COLUMN "oidc_auth" text;--> statement-breakpoint
ALTER TABLE "workspace_identity_providers" ADD CONSTRAINT "workspace_identity_providers_workspace_id_workspaces_id_fk" FOREIGN KEY ("workspace_id") REFERENCES "public"."workspaces"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
CREATE UNIQUE INDEX "workspace_idp_workspace_type_uniq" ON "workspace_identity_providers" USING btree ("workspace_id","type");
