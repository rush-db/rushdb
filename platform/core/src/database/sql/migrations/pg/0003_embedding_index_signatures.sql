ALTER TABLE "embedding_indexes" ADD COLUMN "source_type" text NOT NULL DEFAULT 'managed';
--> statement-breakpoint
ALTER TABLE "embedding_indexes" ADD COLUMN "embedding_mode" text NOT NULL DEFAULT 'text_to_vector';
--> statement-breakpoint
ALTER TABLE "embedding_indexes" ADD COLUMN "similarity_function" text NOT NULL DEFAULT 'cosine';
--> statement-breakpoint
ALTER TABLE "embedding_indexes" ADD COLUMN "vector_slot_key" text NOT NULL DEFAULT 'managed_cosine_1536';
--> statement-breakpoint
ALTER TABLE "embedding_indexes" ADD COLUMN "vector_property_name" text NOT NULL DEFAULT '__emb_managed_cosine_1536';
--> statement-breakpoint
ALTER TABLE "embedding_indexes" ADD COLUMN "query_mode" text NOT NULL DEFAULT 'embed_text';
--> statement-breakpoint
UPDATE "embedding_indexes"
SET
  "source_type" = 'managed',
  "embedding_mode" = 'text_to_vector',
  "similarity_function" = 'cosine',
  "vector_slot_key" = 'managed_cosine_' || "dimensions",
  "vector_property_name" = '__emb_managed_cosine_' || "dimensions",
  "query_mode" = 'embed_text';
--> statement-breakpoint
DROP INDEX IF EXISTS "emb_idx_uniq";
--> statement-breakpoint
CREATE UNIQUE INDEX "emb_idx_signature_uniq" ON "embedding_indexes" ("project_id","property_name","label","source_type","similarity_function","dimensions");