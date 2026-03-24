ALTER TABLE "embedding_indexes" ADD COLUMN "label" text NOT NULL DEFAULT '';
--> statement-breakpoint
CREATE UNIQUE INDEX "emb_idx_uniq" ON "embedding_indexes" ("project_id","property_name","label");
