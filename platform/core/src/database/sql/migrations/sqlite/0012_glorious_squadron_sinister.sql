CREATE TABLE `connector_rejections` (
	`id` text PRIMARY KEY NOT NULL,
	`connector_id` text NOT NULL,
	`project_id` text NOT NULL,
	`batch_id` text NOT NULL,
	`operation_index` integer,
	`source_id_hash` text,
	`code` text NOT NULL,
	`message` text NOT NULL,
	`retryable` integer DEFAULT 0 NOT NULL,
	`occurrence_count` integer DEFAULT 1 NOT NULL,
	`first_seen_at` text NOT NULL,
	`last_seen_at` text NOT NULL,
	`resolved` integer DEFAULT 0 NOT NULL,
	`created_at` text NOT NULL
);
--> statement-breakpoint
CREATE UNIQUE INDEX `connector_rejections_connector_id_code_source_id_hash_unique` ON `connector_rejections` (`connector_id`,`code`,`source_id_hash`);--> statement-breakpoint
ALTER TABLE `connector_rejections` ADD CONSTRAINT `connector_rejections_connector_id_connectors_id_fk` FOREIGN KEY (`connector_id`) REFERENCES `connectors`(`id`) ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE `connector_rejections` ADD CONSTRAINT `connector_rejections_project_id_projects_id_fk` FOREIGN KEY (`project_id`) REFERENCES `projects`(`id`) ON DELETE cascade ON UPDATE no action;
