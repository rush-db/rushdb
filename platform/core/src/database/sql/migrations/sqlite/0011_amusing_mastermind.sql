CREATE TABLE `connector_runs` (
	`id` text PRIMARY KEY NOT NULL,
	`connector_id` text NOT NULL,
	`project_id` text NOT NULL,
	`worker_id` text NOT NULL,
	`trigger` text DEFAULT 'user' NOT NULL,
	`status` text DEFAULT 'running' NOT NULL,
	`phase` text DEFAULT 'starting' NOT NULL,
	`records_read` integer DEFAULT 0 NOT NULL,
	`records_written` integer DEFAULT 0 NOT NULL,
	`records_rejected` integer DEFAULT 0 NOT NULL,
	`error_message` text,
	`started_at` text NOT NULL,
	`completed_at` text,
	`heartbeat_at` text NOT NULL,
	FOREIGN KEY (`connector_id`) REFERENCES `connectors`(`id`) ON UPDATE no action ON DELETE cascade,
	FOREIGN KEY (`project_id`) REFERENCES `projects`(`id`) ON UPDATE no action ON DELETE cascade
);
