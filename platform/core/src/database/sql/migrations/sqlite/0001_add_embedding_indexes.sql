CREATE TABLE `embedding_indexes` (
	`id` text PRIMARY KEY NOT NULL,
	`project_id` text NOT NULL,
	`property_name` text NOT NULL,
	`model_key` text NOT NULL,
	`dimensions` integer NOT NULL,
	`enabled` integer DEFAULT true NOT NULL,
	`status` text DEFAULT 'pending' NOT NULL,
	`created_at` text NOT NULL,
	`updated_at` text NOT NULL,
	FOREIGN KEY (`project_id`) REFERENCES `projects`(`id`) ON UPDATE no action ON DELETE cascade
);
