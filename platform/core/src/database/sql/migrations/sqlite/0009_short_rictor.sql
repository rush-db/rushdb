CREATE TABLE `connector_commands` (
	`id` text PRIMARY KEY NOT NULL,
	`connector_id` text NOT NULL,
	`project_id` text NOT NULL,
	`type` text NOT NULL,
	`status` text DEFAULT 'pending' NOT NULL,
	`payload` text,
	`result` text,
	`error_message` text,
	`requested_by` text,
	`claimed_by` text,
	`created_at` text NOT NULL,
	`claimed_at` text,
	`completed_at` text,
	FOREIGN KEY (`connector_id`) REFERENCES `connectors`(`id`) ON UPDATE no action ON DELETE cascade,
	FOREIGN KEY (`project_id`) REFERENCES `projects`(`id`) ON UPDATE no action ON DELETE cascade
);
