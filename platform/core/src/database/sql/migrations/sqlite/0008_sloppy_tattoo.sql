CREATE TABLE `saved_queries` (
	`id` text PRIMARY KEY NOT NULL,
	`project_id` text NOT NULL,
	`name` text NOT NULL,
	`search_mode` text DEFAULT 'manual' NOT NULL,
	`prompt` text,
	`search_query` text NOT NULL,
	`semantic_index_id` text,
	`created_by` text,
	`created_at` text NOT NULL,
	`updated_at` text NOT NULL,
	FOREIGN KEY (`project_id`) REFERENCES `projects`(`id`) ON UPDATE no action ON DELETE cascade
);
