CREATE TABLE `workspace_identity_providers` (
	`id` text PRIMARY KEY NOT NULL,
	`workspace_id` text NOT NULL,
	`type` text NOT NULL,
	`enabled` integer DEFAULT false NOT NULL,
	`enforced` integer DEFAULT false NOT NULL,
	`domains` text DEFAULT '[]' NOT NULL,
	`default_role` text DEFAULT 'developer' NOT NULL,
	`group_mappings` text,
	`saml_entity_id` text,
	`saml_sso_url` text,
	`saml_certificate` text,
	`oidc_issuer` text,
	`oidc_client_id` text,
	`oidc_client_secret_encrypted` text,
	`created_at` text NOT NULL,
	`updated_at` text NOT NULL,
	FOREIGN KEY (`workspace_id`) REFERENCES `workspaces`(`id`) ON UPDATE no action ON DELETE cascade
);
--> statement-breakpoint
CREATE UNIQUE INDEX `workspace_idp_workspace_type_uniq` ON `workspace_identity_providers` (`workspace_id`,`type`);--> statement-breakpoint
ALTER TABLE `users` ADD `saml_auth` text;--> statement-breakpoint
ALTER TABLE `users` ADD `oidc_auth` text;
