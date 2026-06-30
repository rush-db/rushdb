ALTER TABLE `projects` RENAME COLUMN `ontology_cache` TO `schema_cache`;--> statement-breakpoint
ALTER TABLE `projects` RENAME COLUMN `ontology_cached_at` TO `schema_cached_at`;
