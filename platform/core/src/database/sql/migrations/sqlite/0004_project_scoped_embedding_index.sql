DROP INDEX IF EXISTS `emb_idx_signature_uniq`;--> statement-breakpoint
CREATE UNIQUE INDEX `emb_idx_signature_uniq` ON `embedding_indexes` (`project_id`,`property_name`,`label`,`source_type`,`similarity_function`,`dimensions`);
