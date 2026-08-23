CREATE TABLE `usage_event_outbox` (
	`id` text PRIMARY KEY NOT NULL,
	`idempotency_key` text NOT NULL,
	`workspace_id` text NOT NULL,
	`project_id` text,
	`operation_class` text NOT NULL,
	`payload` text NOT NULL,
	`status` text DEFAULT 'pending' NOT NULL,
	`attempt_count` integer DEFAULT 0 NOT NULL,
	`next_attempt_at` text NOT NULL,
	`last_error` text,
	`created_at` text NOT NULL,
	`acknowledged_at` text
);
--> statement-breakpoint
CREATE UNIQUE INDEX `usage_event_outbox_idempotency_key_idx` ON `usage_event_outbox` (`idempotency_key`);--> statement-breakpoint
CREATE INDEX `usage_event_outbox_delivery_idx` ON `usage_event_outbox` (`status`,`next_attempt_at`);