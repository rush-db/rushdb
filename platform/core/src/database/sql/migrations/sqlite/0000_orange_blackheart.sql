CREATE TABLE `embedding_indexes` (
	`id` text PRIMARY KEY NOT NULL,
	`project_id` text NOT NULL,
	`label` text DEFAULT '' NOT NULL,
	`property_name` text NOT NULL,
	`model_key` text NOT NULL,
	`source_type` text DEFAULT 'managed' NOT NULL,
	`similarity_function` text DEFAULT 'cosine' NOT NULL,
	`dimensions` integer NOT NULL,
	`vector_property_name` text NOT NULL,
	`enabled` integer DEFAULT true NOT NULL,
	`status` text DEFAULT 'pending' NOT NULL,
	`created_at` text NOT NULL,
	`updated_at` text NOT NULL,
	FOREIGN KEY (`project_id`) REFERENCES `projects`(`id`) ON UPDATE no action ON DELETE cascade
);
--> statement-breakpoint
CREATE UNIQUE INDEX `emb_idx_signature_uniq` ON `embedding_indexes` (`project_id`,`property_name`,`label`,`source_type`,`similarity_function`,`dimensions`);--> statement-breakpoint
CREATE TABLE `oauth_auth_requests` (
	`id` text PRIMARY KEY NOT NULL,
	`client_id` text NOT NULL,
	`redirect_uri` text NOT NULL,
	`scope` text,
	`resource` text,
	`code_challenge` text NOT NULL,
	`code_challenge_method` text NOT NULL,
	`state` text,
	`created` text NOT NULL,
	`expires_at` text NOT NULL,
	FOREIGN KEY (`client_id`) REFERENCES `oauth_clients`(`id`) ON UPDATE no action ON DELETE cascade
);
--> statement-breakpoint
CREATE TABLE `oauth_clients` (
	`id` text PRIMARY KEY NOT NULL,
	`client_name` text NOT NULL,
	`redirect_uris` text NOT NULL,
	`token_endpoint_auth_method` text,
	`application_type` text,
	`created` text NOT NULL
);
--> statement-breakpoint
CREATE UNIQUE INDEX `uq_oauth_client_name_uris` ON `oauth_clients` (`client_name`,`redirect_uris`);--> statement-breakpoint
CREATE TABLE `oauth_codes` (
	`id` text PRIMARY KEY NOT NULL,
	`consent_id` text NOT NULL,
	`client_id` text NOT NULL,
	`redirect_uri` text NOT NULL,
	`resource` text,
	`scope` text,
	`code_challenge` text NOT NULL,
	`code_challenge_method` text NOT NULL,
	`created` text NOT NULL,
	`expires_at` text NOT NULL,
	FOREIGN KEY (`consent_id`) REFERENCES `oauth_consents`(`id`) ON UPDATE no action ON DELETE cascade
);
--> statement-breakpoint
CREATE TABLE `oauth_consents` (
	`id` text PRIMARY KEY NOT NULL,
	`user_id` text NOT NULL,
	`client_id` text NOT NULL,
	`project_id` text NOT NULL,
	`resource` text,
	`scope` text NOT NULL,
	`created` text NOT NULL,
	`revoked_at` text,
	FOREIGN KEY (`client_id`) REFERENCES `oauth_clients`(`id`) ON UPDATE no action ON DELETE cascade
);
--> statement-breakpoint
CREATE TABLE `project_access` (
	`project_id` text NOT NULL,
	`user_id` text NOT NULL,
	`role` text NOT NULL,
	`since` text NOT NULL,
	PRIMARY KEY(`project_id`, `user_id`),
	FOREIGN KEY (`project_id`) REFERENCES `projects`(`id`) ON UPDATE no action ON DELETE cascade,
	FOREIGN KEY (`user_id`) REFERENCES `users`(`id`) ON UPDATE no action ON DELETE cascade
);
--> statement-breakpoint
CREATE TABLE `projects` (
	`id` text PRIMARY KEY NOT NULL,
	`workspace_id` text NOT NULL,
	`name` text NOT NULL,
	`description` text,
	`created` text NOT NULL,
	`edited` text,
	`deleted` text,
	`status` text,
	`stats` text,
	`custom_db` text,
	`ontology_cache` text,
	`ontology_cached_at` text,
	FOREIGN KEY (`workspace_id`) REFERENCES `workspaces`(`id`) ON UPDATE no action ON DELETE cascade
);
--> statement-breakpoint
CREATE TABLE `tokens` (
	`id` text PRIMARY KEY NOT NULL,
	`project_id` text NOT NULL,
	`name` text NOT NULL,
	`expiration` integer NOT NULL,
	`created` text NOT NULL,
	`description` text,
	`value` text NOT NULL,
	`prefix_value` text,
	`consent_id` text,
	`level` text DEFAULT 'write' NOT NULL,
	FOREIGN KEY (`project_id`) REFERENCES `projects`(`id`) ON UPDATE no action ON DELETE cascade
);
--> statement-breakpoint
CREATE TABLE `users` (
	`id` text PRIMARY KEY NOT NULL,
	`login` text NOT NULL,
	`is_email` integer DEFAULT false NOT NULL,
	`first_name` text,
	`last_name` text,
	`confirmed` integer DEFAULT false NOT NULL,
	`status` text,
	`created` text NOT NULL,
	`edited` text,
	`last_activity` text,
	`google_auth` text,
	`github_auth` text,
	`password` text,
	`deleted_date` text,
	`settings` text
);
--> statement-breakpoint
CREATE UNIQUE INDEX `users_login_unique` ON `users` (`login`);--> statement-breakpoint
CREATE TABLE `workspace_invites` (
	`id` text PRIMARY KEY NOT NULL,
	`workspace_id` text NOT NULL,
	`email` text NOT NULL,
	`created_at` text NOT NULL,
	FOREIGN KEY (`workspace_id`) REFERENCES `workspaces`(`id`) ON UPDATE no action ON DELETE cascade
);
--> statement-breakpoint
CREATE TABLE `workspace_members` (
	`workspace_id` text NOT NULL,
	`user_id` text NOT NULL,
	`role` text NOT NULL,
	`since` text NOT NULL,
	PRIMARY KEY(`workspace_id`, `user_id`),
	FOREIGN KEY (`workspace_id`) REFERENCES `workspaces`(`id`) ON UPDATE no action ON DELETE cascade,
	FOREIGN KEY (`user_id`) REFERENCES `users`(`id`) ON UPDATE no action ON DELETE cascade
);
--> statement-breakpoint
CREATE TABLE `workspaces` (
	`id` text PRIMARY KEY NOT NULL,
	`name` text NOT NULL,
	`created` text NOT NULL,
	`edited` text,
	`stats` text
);
