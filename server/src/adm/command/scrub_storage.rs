use std::sync::Arc;

use anyhow::Result;
use clap::Parser;
use futures::future::join_all;
use sea_orm::entity::prelude::*;
use tokio::sync::Semaphore;
use tracing::instrument;

use crate::Opts;
use attic_server::config::Config;
use attic_server::database::entity::chunk::{self, ChunkState, Entity as Chunk};
use attic_server::database::entity::chunkref::{self, Entity as ChunkRef};
use attic_server::database::entity::nar::{self, Entity as Nar, NarState};
use attic_server::StateInner;

const PARALLEL_CHECKS: usize = 20;

#[derive(Debug, Parser)]
pub struct ScrubStorage {
    /// What to check: "all", "nar", or "chunk".
    #[clap(long = "type", value_name = "TYPE", value_enum, default_value = "all")]
    pub scrub_type: ScrubType,
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum ScrubType {
    /// Check both NARs and chunks.
    All,
    /// Only check NAR completeness.
    Nar,
    /// Only check chunk storage.
    Chunk,
}

pub async fn run(config: Config, opts: Opts) -> Result<()> {
    let sub = opts.command.as_scrub_storage().unwrap();

    let state = Arc::new(StateInner::new(config).await);

    let mut total_errors = 0usize;

    match sub.scrub_type {
        ScrubType::All => {
            total_errors += check_nar_completeness(&state).await?;
            total_errors += check_chunk_storage(&state).await?;
        }
        ScrubType::Nar => {
            total_errors += check_nar_completeness(&state).await?;
        }
        ScrubType::Chunk => {
            total_errors += check_chunk_storage(&state).await?;
        }
    }

    if total_errors == 0 {
        eprintln!("No errors found.");
    } else {
        eprintln!("Found {total_errors} total errors.");
    }

    Ok(())
}

#[instrument(skip_all)]
async fn check_nar_completeness(state: &StateInner) -> Result<usize> {
    let db = state.database().await?;
    let storage = state.storage().await?;

    eprintln!(
        "Checking NAR completeness (verifying all chunks exist and sizes match for each NAR)..."
    );

    let valid_nars: Vec<nar::Model> = Nar::find()
        .filter(nar::Column::State.eq(NarState::Valid))
        .all(db)
        .await?;

    eprintln!("Found {} NARs to check", valid_nars.len());

    let semaphore = Arc::new(Semaphore::new(PARALLEL_CHECKS));

    let futures = valid_nars.into_iter().map(|nar| {
        let db = db.clone();
        let storage = storage.clone();
        let semaphore = semaphore.clone();
        async move {
            let _permit = semaphore.acquire().await?;

            let chunkrefs: Vec<chunkref::Model> = ChunkRef::find()
                .filter(chunkref::Column::NarId.eq(nar.id))
                .all(&db)
                .await?;

            let mut missing_chunks = 0;
            let mut size_mismatches = 0;
            for chunkref in chunkrefs {
                if let Some(chunk_id) = chunkref.chunk_id {
                    let valid_chunk: Option<chunk::Model> = Chunk::find()
                        .filter(chunk::Column::Id.eq(chunk_id))
                        .filter(chunk::Column::State.eq(ChunkState::Valid))
                        .one(&db)
                        .await?;

                    if let Some(chunk) = valid_chunk {
                        let actual_size = storage.file_size(&chunk.remote_file.0).await;
                        match actual_size {
                            Ok(actual_size) => {
                                if let Some(expected_size) = chunk.file_size {
                                    if expected_size != actual_size {
                                        tracing::warn!(
                                            "Chunk {} (ID {}) for NAR {} (ID {}) has size mismatch: expected {}, got {}",
                                            chunk.chunk_hash,
                                            chunk.id,
                                            nar.nar_hash,
                                            nar.id,
                                            expected_size,
                                            actual_size
                                        );
                                        size_mismatches += 1;
                                    }
                                }
                            }
                            Err(_) => {
                                tracing::warn!(
                                    "Chunk {} (ID {}) for NAR {} (ID {}) exists in database but not in storage",
                                    chunk.chunk_hash,
                                    chunk.id,
                                    nar.nar_hash,
                                    nar.id
                                );
                                missing_chunks += 1;
                            }
                        }
                    } else {
                        missing_chunks += 1;
                    }
                } else {
                    missing_chunks += 1;
                }
            }

            if missing_chunks > 0 || size_mismatches > 0 {
                tracing::warn!(
                    "NAR {} (ID {}) has {} missing chunks and {} size mismatches",
                    nar.nar_hash,
                    nar.id,
                    missing_chunks,
                    size_mismatches
                );
            }

            Ok::<_, anyhow::Error>((nar.id, missing_chunks + size_mismatches))
        }
    });

    let results = join_all(futures).await;

    let mut incomplete_count = 0;
    for result in results {
        let (_id, issue_count) = result?;
        if issue_count > 0 {
            incomplete_count += 1;
        }
    }

    if incomplete_count > 0 {
        eprintln!(
            "WARNING: {} NARs are incomplete (missing chunks or size mismatches in storage)!",
            incomplete_count
        );
    } else {
        eprintln!("All NARs are complete (all chunks present in storage with correct sizes).");
    }

    Ok(incomplete_count)
}

#[instrument(skip_all)]
async fn check_chunk_storage(state: &StateInner) -> Result<usize> {
    let db = state.database().await?;
    let storage = state.storage().await?;

    eprintln!("Checking chunk storage (existence and size)...");

    let valid_chunks: Vec<chunk::Model> = Chunk::find()
        .filter(chunk::Column::State.eq(ChunkState::Valid))
        .all(db)
        .await?;

    eprintln!("Found {} chunks to check", valid_chunks.len());

    let semaphore = Arc::new(Semaphore::new(PARALLEL_CHECKS));

    #[derive(Debug)]
    enum ChunkIssue {
        Missing,
        SizeMismatch { expected: i64, actual: i64 },
    }

    let futures = valid_chunks.into_iter().map(|chunk| {
        let storage = storage.clone();
        let semaphore = semaphore.clone();
        async move {
            let _permit = semaphore.acquire().await?;
            let actual_size = storage.file_size(&chunk.remote_file.0).await;

            let issue: Option<ChunkIssue> = match actual_size {
                Ok(actual_size) => {
                    if let Some(expected_size) = chunk.file_size {
                        if expected_size != actual_size {
                            Some(ChunkIssue::SizeMismatch {
                                expected: expected_size,
                                actual: actual_size,
                            })
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }
                Err(_) => Some(ChunkIssue::Missing),
            };

            if let Some(ref issue) = issue {
                match issue {
                    ChunkIssue::Missing => {
                        tracing::warn!(
                            "Chunk {} (ID {}) exists in database but not in storage",
                            chunk.chunk_hash,
                            chunk.id
                        );
                    }
                    ChunkIssue::SizeMismatch { expected, actual } => {
                        tracing::warn!(
                            "Chunk {} (ID {}) has size mismatch: expected {}, got {}",
                            chunk.chunk_hash,
                            chunk.id,
                            expected,
                            actual
                        );
                    }
                }
            }

            Ok::<_, anyhow::Error>((chunk.id, issue))
        }
    });

    let results = join_all(futures).await;

    let mut missing_count = 0;
    let mut size_mismatch_count = 0;
    for result in results {
        let (_id, issue) = result?;
        if let Some(ChunkIssue::Missing) = issue {
            missing_count += 1;
        } else if let Some(ChunkIssue::SizeMismatch { .. }) = issue {
            size_mismatch_count += 1;
        }
    }

    if missing_count > 0 {
        eprintln!(
            "WARNING: {} chunks are missing from storage but exist in the database!",
            missing_count
        );
    }
    if size_mismatch_count > 0 {
        eprintln!(
            "WARNING: {} chunks have size mismatches between database and storage!",
            size_mismatch_count
        );
    }
    if missing_count == 0 && size_mismatch_count == 0 {
        eprintln!("All chunks are present in storage with correct sizes.");
    }

    Ok(missing_count + size_mismatch_count)
}
