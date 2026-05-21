CREATE TABLE `oauth_refresh_tokens` (
	`id` text PRIMARY KEY NOT NULL,
	`consent_id` text NOT NULL,
	`client_id` text NOT NULL,
	`user_id` text NOT NULL,
	`project_id` text NOT NULL,
	`scope` text NOT NULL,
	`created_at` text NOT NULL,
	`expires_at` text NOT NULL,
	FOREIGN KEY (`consent_id`) REFERENCES `oauth_consents`(`id`) ON UPDATE no action ON DELETE cascade
);
