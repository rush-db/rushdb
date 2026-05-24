export enum KuOperation {
  ENTITY_CREATED = 'entity_created',
  RELATIONSHIP_CREATED = 'relationship_created',
  HYPERPROPERTY_CREATED = 'hyperproperty_created',
  EMBEDDING_GENERATED = 'embedding_generated',
  KNOWLEDGE_DELETED = 'knowledge_deleted',
  /** Emitted when a compute-intensive operation runs (raw Cypher, vector similarity
   *  search, multi-hop traversals). Reflects real server compute cost that scales
   *  with dataset size rather than data written. Standard reads never emit this. */
  COMPUTE_OPERATION = 'compute_operation',
  /** Emitted daily by the storage-footprint scheduler for every project.
   *  Count = current number of records stored. The billing service uses
   *  this to apply a prorated daily charge for ongoing data-at-rest. */
  STORAGE_FOOTPRINT = 'storage_footprint',
  /** Emitted each time an LLM-powered relationship pattern analysis runs
   *  (either triggered manually by the user or via the background scheduler).
   *  Reflects the compute cost of ontology-to-candidate LLM inference. */
  RELATIONSHIP_ANALYSIS = 'relationship_analysis'
}
