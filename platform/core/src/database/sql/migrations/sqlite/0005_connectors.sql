CREATE TABLE IF NOT EXISTS `connectors` (
  `id` text PRIMARY KEY NOT NULL,
  `project_id` text NOT NULL,
  `name` text NOT NULL,
  `type` text NOT NULL,
  `config` text NOT NULL,
  `transform` text NOT NULL,
  `status` text DEFAULT 'paused' NOT NULL,
  `last_error` text,
  `lag_ms` integer,
  `stats` text,
  `created_by` text,
  `created_at` text NOT NULL,
  `updated_at` text NOT NULL,
  FOREIGN KEY (`project_id`) REFERENCES `projects`(`id`) ON UPDATE no action ON DELETE cascade
);

CREATE TABLE IF NOT EXISTS `connector_secrets` (
  `connector_id` text PRIMARY KEY NOT NULL,
  `provider` text DEFAULT 'local' NOT NULL,
  `secret_ref` text,
  `ciphertext` text,
  `created_at` text NOT NULL,
  `updated_at` text NOT NULL,
  FOREIGN KEY (`connector_id`) REFERENCES `connectors`(`id`) ON UPDATE no action ON DELETE cascade
);

CREATE TABLE IF NOT EXISTS `connector_offsets` (
  `connector_id` text NOT NULL,
  `partition` text NOT NULL,
  `position` text NOT NULL,
  `updated_at` text NOT NULL,
  PRIMARY KEY(`connector_id`, `partition`),
  FOREIGN KEY (`connector_id`) REFERENCES `connectors`(`id`) ON UPDATE no action ON DELETE cascade
);

CREATE TABLE IF NOT EXISTS `connector_events` (
  `id` text PRIMARY KEY NOT NULL,
  `connector_id` text NOT NULL,
  `project_id` text NOT NULL,
  `level` text DEFAULT 'info' NOT NULL,
  `type` text NOT NULL,
  `message` text NOT NULL,
  `metadata` text,
  `created_at` text NOT NULL,
  FOREIGN KEY (`connector_id`) REFERENCES `connectors`(`id`) ON UPDATE no action ON DELETE cascade,
  FOREIGN KEY (`project_id`) REFERENCES `projects`(`id`) ON UPDATE no action ON DELETE cascade
);

CREATE TABLE IF NOT EXISTS `connector_leases` (
  `connector_id` text PRIMARY KEY NOT NULL,
  `worker_id` text NOT NULL,
  `lease_until` text NOT NULL,
  `heartbeat_at` text NOT NULL,
  `created_at` text NOT NULL,
  `updated_at` text NOT NULL,
  FOREIGN KEY (`connector_id`) REFERENCES `connectors`(`id`) ON UPDATE no action ON DELETE cascade
);
