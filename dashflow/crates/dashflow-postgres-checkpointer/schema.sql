-- DashFlow Checkpoint Storage Schema for PostgreSQL
-- This schema stores checkpoints for graph execution state persistence.

CREATE TABLE IF NOT EXISTS dashflow_checkpoints (
    -- Unique identifier for this checkpoint
    checkpoint_id TEXT PRIMARY KEY,
    
    -- Thread/execution ID this checkpoint belongs to
    thread_id TEXT NOT NULL,
    
    -- The graph state at this point (bincode-encoded)
    state BYTEA NOT NULL,
    
    -- Node that was just executed (or about to be executed)
    node TEXT NOT NULL,
    
    -- Timestamp when checkpoint was created (Unix timestamp in nanoseconds)
    timestamp BIGINT NOT NULL,
    
    -- Parent checkpoint ID (for tracking execution history)
    parent_id TEXT,
    
    -- Metadata about this checkpoint (JSON)
    metadata JSONB
);

-- Index for efficient thread_id lookups
CREATE INDEX IF NOT EXISTS idx_dashflow_checkpoints_thread_id 
    ON dashflow_checkpoints (thread_id);

-- Index for efficient timestamp-based queries (e.g., get latest)
CREATE INDEX IF NOT EXISTS idx_dashflow_checkpoints_timestamp 
    ON dashflow_checkpoints (timestamp);

-- Composite index for thread_id + timestamp (optimizes get_latest queries)
CREATE INDEX IF NOT EXISTS idx_dashflow_checkpoints_thread_timestamp 
    ON dashflow_checkpoints (thread_id, timestamp DESC);
