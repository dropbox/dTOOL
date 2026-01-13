//! Character and theme analysis for the Superhuman Librarian
//!
//! This module extracts and analyzes:
//! - Characters and their mentions across the corpus
//! - Character relationships (family, romantic, antagonistic)
//! - Themes and supporting evidence
//!
//! ## Example
//!
//! ```ignore
//! let analyzer = BookAnalyzer::new(searcher);
//! let characters = analyzer.extract_characters("Pride and Prejudice").await?;
//! let relationships = analyzer.find_relationships(&characters).await?;
//! let themes = analyzer.extract_themes("1342").await?;
//! ```

use crate::search::{HybridSearcher, SearchFilters, SearchResult};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Arc;
use tracing::{info, instrument};

/// Known characters with aliases for major books
/// Pre-defined to avoid expensive NER processing
static KNOWN_CHARACTERS: &[(&str, &[&str], &[&str])] = &[
    // Pride and Prejudice (Gutenberg ID: 1342)
    (
        "Elizabeth Bennet",
        &["Elizabeth", "Lizzy", "Eliza", "Miss Elizabeth"],
        &["1342"],
    ),
    (
        "Fitzwilliam Darcy",
        &["Darcy", "Mr. Darcy", "Fitzwilliam"],
        &["1342"],
    ),
    ("Jane Bennet", &["Jane", "Miss Bennet"], &["1342"]),
    ("Mr. Bingley", &["Bingley", "Charles Bingley"], &["1342"]),
    ("Mr. Bennet", &["Mr. Bennet"], &["1342"]),
    ("Mrs. Bennet", &["Mrs. Bennet"], &["1342"]),
    ("Lydia Bennet", &["Lydia"], &["1342"]),
    ("Mr. Wickham", &["Wickham", "George Wickham"], &["1342"]),
    (
        "Lady Catherine de Bourgh",
        &["Lady Catherine", "Lady Catherine de Bourgh"],
        &["1342"],
    ),
    ("Mr. Collins", &["Collins", "Mr. Collins"], &["1342"]),
    // Moby Dick (Gutenberg ID: 2701)
    (
        "Captain Ahab",
        &["Ahab", "Captain Ahab", "the Captain"],
        &["2701"],
    ),
    ("Ishmael", &["Ishmael"], &["2701"]),
    ("Queequeg", &["Queequeg"], &["2701"]),
    ("Starbuck", &["Starbuck"], &["2701"]),
    (
        "Moby Dick",
        &["Moby Dick", "the White Whale", "white whale"],
        &["2701"],
    ),
    // Frankenstein (Gutenberg ID: 84)
    (
        "Victor Frankenstein",
        &["Victor", "Frankenstein", "Victor Frankenstein"],
        &["84"],
    ),
    (
        "The Creature",
        &["the creature", "the monster", "monster", "daemon", "wretch"],
        &["84"],
    ),
    ("Elizabeth Lavenza", &["Elizabeth"], &["84"]),
    ("Henry Clerval", &["Clerval", "Henry"], &["84"]),
    ("Robert Walton", &["Walton", "Robert Walton"], &["84"]),
    // Hamlet (Gutenberg ID: 1524)
    ("Hamlet", &["Hamlet", "the Prince"], &["1524"]),
    ("Claudius", &["Claudius", "the King"], &["1524"]),
    ("Gertrude", &["Gertrude", "the Queen"], &["1524"]),
    ("Ophelia", &["Ophelia"], &["1524"]),
    ("Polonius", &["Polonius"], &["1524"]),
    ("Laertes", &["Laertes"], &["1524"]),
    ("Horatio", &["Horatio"], &["1524"]),
    // A Tale of Two Cities (Gutenberg ID: 98)
    (
        "Sydney Carton",
        &["Carton", "Sydney Carton", "Sydney"],
        &["98"],
    ),
    ("Charles Darnay", &["Darnay", "Charles Darnay"], &["98"]),
    ("Lucie Manette", &["Lucie", "Lucie Manette"], &["98"]),
    (
        "Doctor Manette",
        &["Doctor Manette", "Dr. Manette"],
        &["98"],
    ),
    ("Madame Defarge", &["Madame Defarge", "Defarge"], &["98"]),
];

/// Known themes with search keywords
static KNOWN_THEMES: &[(&str, &[&str], &str)] = &[
    // Universal themes
    (
        "Pride",
        &["pride", "proud", "arrogant", "arrogance", "haughty"],
        "The excessive belief in one's own worth or abilities",
    ),
    (
        "Prejudice",
        &["prejudice", "prejudiced", "bias", "biased", "judgment"],
        "Preconceived opinion not based on reason or experience",
    ),
    (
        "Love",
        &[
            "love",
            "affection",
            "beloved",
            "loving",
            "passion",
            "romantic",
        ],
        "Deep affection and attachment between people",
    ),
    (
        "Revenge",
        &["revenge", "vengeance", "avenge", "retribution"],
        "The act of inflicting punishment in return for injury",
    ),
    (
        "Obsession",
        &["obsession", "obsessed", "consumed", "fixation", "monomania"],
        "An unhealthy preoccupation with something",
    ),
    (
        "Death",
        &["death", "dead", "die", "dying", "mortality", "grave"],
        "The end of life and its implications",
    ),
    (
        "Social Class",
        &["class", "rank", "fortune", "estate", "society", "gentleman"],
        "The hierarchical distinctions between groups",
    ),
    (
        "Nature vs Nurture",
        &["nature", "creature", "creation", "born", "made"],
        "Whether character is inherited or learned",
    ),
    (
        "Isolation",
        &["alone", "isolation", "solitude", "lonely", "abandoned"],
        "Being separated from others",
    ),
    (
        "Ambition",
        &["ambition", "ambitious", "aspire", "glory", "achieve"],
        "Strong desire for success or achievement",
    ),
    (
        "Betrayal",
        &["betray", "betrayal", "treachery", "deceive", "false"],
        "Violation of trust or confidence",
    ),
    (
        "Redemption",
        &["redeem", "redemption", "save", "forgiveness", "atone"],
        "Deliverance from sin or error",
    ),
    (
        "Justice",
        &["justice", "just", "fair", "right", "wrong", "punishment"],
        "Fair treatment and moral rightness",
    ),
    (
        "Fate",
        &["fate", "destiny", "fortune", "doom", "predestined"],
        "The development of events outside one's control",
    ),
];

/// A character identified in the corpus
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Character {
    /// Primary name of the character
    pub name: String,

    /// Alternative names/aliases
    pub aliases: Vec<String>,

    /// Book IDs where this character appears
    pub book_ids: Vec<String>,

    /// Number of mentions found
    pub mention_count: usize,

    /// Sample chunks where character is mentioned
    pub evidence_chunks: Vec<EvidenceChunk>,
}

/// A chunk of text serving as evidence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceChunk {
    /// The text content
    pub content: String,

    /// Book ID
    pub book_id: String,

    /// Book title
    pub book_title: String,

    /// Chunk index within the book
    pub chunk_index: i64,

    /// Relevance score
    pub score: f32,
}

impl From<&SearchResult> for EvidenceChunk {
    fn from(result: &SearchResult) -> Self {
        Self {
            content: result.content.clone(),
            book_id: result.book_id.clone(),
            book_title: result.title.clone(),
            chunk_index: result.chunk_index,
            score: result.score,
        }
    }
}

/// Types of character relationships
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RelationshipType {
    /// Family relationship (parent, sibling, etc.)
    Family,
    /// Romantic relationship
    Romantic,
    /// Friendship
    Friend,
    /// Antagonist/enemy relationship
    Antagonist,
    /// Professional relationship
    Professional,
    /// Other or unspecified
    Other,
}

impl std::fmt::Display for RelationshipType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RelationshipType::Family => write!(f, "Family"),
            RelationshipType::Romantic => write!(f, "Romantic"),
            RelationshipType::Friend => write!(f, "Friend"),
            RelationshipType::Antagonist => write!(f, "Antagonist"),
            RelationshipType::Professional => write!(f, "Professional"),
            RelationshipType::Other => write!(f, "Other"),
        }
    }
}

/// A relationship between two characters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Relationship {
    /// First character name
    pub character1: String,

    /// Second character name
    pub character2: String,

    /// Type of relationship
    pub relationship_type: RelationshipType,

    /// Brief description of the relationship
    pub description: String,

    /// Evidence chunks showing the relationship
    pub evidence: Vec<EvidenceChunk>,
}

/// A theme identified in a book
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Theme {
    /// Name of the theme
    pub name: String,

    /// Description of the theme
    pub description: String,

    /// Keywords associated with this theme
    pub keywords: Vec<String>,

    /// Evidence chunks supporting this theme
    pub evidence: Vec<EvidenceChunk>,

    /// Relevance score (based on evidence count and scores)
    pub relevance_score: f32,
}

/// Result of character analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharacterAnalysis {
    /// Book ID analyzed
    pub book_id: String,

    /// Book title
    pub book_title: String,

    /// Characters found
    pub characters: Vec<Character>,

    /// Relationships between characters
    pub relationships: Vec<Relationship>,
}

/// Result of theme analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeAnalysis {
    /// Book ID analyzed
    pub book_id: String,

    /// Book title
    pub book_title: String,

    /// Themes found
    pub themes: Vec<Theme>,
}

/// Analyzer for extracting characters and themes from books
pub struct BookAnalyzer {
    searcher: Arc<HybridSearcher>,
}

impl BookAnalyzer {
    /// Create a new book analyzer
    pub fn new(searcher: Arc<HybridSearcher>) -> Self {
        Self { searcher }
    }

    /// Extract characters from a specific book
    #[instrument(skip(self), fields(book_id = %book_id))]
    pub async fn extract_characters(&self, book_id: &str) -> Result<CharacterAnalysis> {
        info!("Extracting characters for book {}", book_id);

        // Get known characters for this book
        let book_characters: Vec<_> = KNOWN_CHARACTERS
            .iter()
            .filter(|(_, _, books)| books.contains(&book_id))
            .collect();

        let mut characters = Vec::new();
        let mut book_title = String::new();

        for (name, aliases, _) in book_characters {
            // Search for character mentions
            let filters = SearchFilters {
                book_id: Some(book_id.to_string()),
                ..Default::default()
            };

            // Search using primary name
            let results = self.searcher.search_filtered(name, &filters, 10).await?;

            if let Some(first) = results.first() {
                book_title = first.title.clone();
            }

            // Count mentions and collect evidence
            let mut mention_count = results.len();
            let mut evidence_chunks: Vec<EvidenceChunk> =
                results.iter().take(3).map(EvidenceChunk::from).collect();

            // Also search for aliases
            for alias in *aliases {
                if alias != name {
                    let alias_results = self.searcher.search_filtered(alias, &filters, 5).await?;
                    mention_count += alias_results.len();

                    // Add unique evidence
                    for result in alias_results.iter().take(2) {
                        if !evidence_chunks
                            .iter()
                            .any(|e| e.chunk_index == result.chunk_index)
                        {
                            evidence_chunks.push(EvidenceChunk::from(result));
                        }
                    }
                }
            }

            if mention_count > 0 {
                characters.push(Character {
                    name: name.to_string(),
                    aliases: aliases.iter().map(|s| s.to_string()).collect(),
                    book_ids: vec![book_id.to_string()],
                    mention_count,
                    evidence_chunks,
                });
            }
        }

        // Sort by mention count
        characters.sort_by(|a, b| b.mention_count.cmp(&a.mention_count));

        metrics::counter!("librarian_analysis_total", "type" => "characters").increment(1);

        Ok(CharacterAnalysis {
            book_id: book_id.to_string(),
            book_title,
            characters,
            relationships: Vec::new(), // Will be populated by find_relationships
        })
    }

    /// Find relationships between characters in a book
    #[instrument(skip(self, analysis), fields(book_id = %analysis.book_id))]
    pub async fn find_relationships(&self, analysis: &mut CharacterAnalysis) -> Result<()> {
        info!(
            "Finding relationships for {} characters",
            analysis.characters.len()
        );

        let mut relationships = Vec::new();

        // Define known relationships for major books
        let known_relationships = get_known_relationships(&analysis.book_id);

        for (char1, char2, rel_type, description) in known_relationships {
            // Verify both characters exist in analysis
            let has_char1 = analysis
                .characters
                .iter()
                .any(|c| c.name == char1 || c.aliases.contains(&char1.to_string()));
            let has_char2 = analysis
                .characters
                .iter()
                .any(|c| c.name == char2 || c.aliases.contains(&char2.to_string()));

            if !has_char1 || !has_char2 {
                continue;
            }

            // Search for evidence of their relationship
            let search_query = format!("{} {}", char1, char2);
            let filters = SearchFilters {
                book_id: Some(analysis.book_id.clone()),
                ..Default::default()
            };

            let results = self
                .searcher
                .search_filtered(&search_query, &filters, 5)
                .await?;

            let evidence: Vec<EvidenceChunk> =
                results.iter().take(3).map(EvidenceChunk::from).collect();

            if !evidence.is_empty() {
                relationships.push(Relationship {
                    character1: char1.to_string(),
                    character2: char2.to_string(),
                    relationship_type: rel_type,
                    description: description.to_string(),
                    evidence,
                });
            }
        }

        analysis.relationships = relationships;

        metrics::counter!("librarian_analysis_total", "type" => "relationships").increment(1);

        Ok(())
    }

    /// Extract themes from a book
    #[instrument(skip(self), fields(book_id = %book_id))]
    pub async fn extract_themes(&self, book_id: &str) -> Result<ThemeAnalysis> {
        info!("Extracting themes for book {}", book_id);

        let mut themes = Vec::new();
        let mut book_title = String::new();

        for (name, keywords, description) in KNOWN_THEMES {
            // Search for theme keywords in this book
            let filters = SearchFilters {
                book_id: Some(book_id.to_string()),
                ..Default::default()
            };

            let mut all_evidence = Vec::new();
            let mut seen_chunks = HashSet::new();

            for keyword in *keywords {
                let results = self.searcher.search_filtered(keyword, &filters, 5).await?;

                if book_title.is_empty() {
                    if let Some(first) = results.first() {
                        book_title = first.title.clone();
                    }
                }

                for result in results {
                    if !seen_chunks.contains(&result.chunk_index) {
                        seen_chunks.insert(result.chunk_index);
                        all_evidence.push(EvidenceChunk::from(&result));
                    }
                }
            }

            if !all_evidence.is_empty() {
                // Calculate relevance score based on evidence
                let relevance_score = calculate_theme_relevance(&all_evidence);

                // Sort evidence by score
                let mut evidence = all_evidence;
                evidence.sort_by(|a, b| {
                    b.score
                        .partial_cmp(&a.score)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
                evidence.truncate(5); // Keep top 5

                themes.push(Theme {
                    name: name.to_string(),
                    description: description.to_string(),
                    keywords: keywords.iter().map(|s| s.to_string()).collect(),
                    evidence,
                    relevance_score,
                });
            }
        }

        // Sort themes by relevance
        themes.sort_by(|a, b| {
            b.relevance_score
                .partial_cmp(&a.relevance_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        metrics::counter!("librarian_analysis_total", "type" => "themes").increment(1);

        Ok(ThemeAnalysis {
            book_id: book_id.to_string(),
            book_title,
            themes,
        })
    }

    /// Get a complete analysis of a book (characters, relationships, and themes)
    pub async fn analyze_book(&self, book_id: &str) -> Result<(CharacterAnalysis, ThemeAnalysis)> {
        let mut character_analysis = self.extract_characters(book_id).await?;
        self.find_relationships(&mut character_analysis).await?;
        let theme_analysis = self.extract_themes(book_id).await?;

        Ok((character_analysis, theme_analysis))
    }
}

/// Calculate theme relevance score from evidence
fn calculate_theme_relevance(evidence: &[EvidenceChunk]) -> f32 {
    if evidence.is_empty() {
        return 0.0;
    }

    // Combine count and average score
    let count_factor = (evidence.len() as f32).sqrt() / 3.0; // Normalize by sqrt
    let avg_score: f32 = evidence.iter().map(|e| e.score).sum::<f32>() / evidence.len() as f32;

    // Final score: weighted combination
    (count_factor * 0.4 + avg_score * 0.6).min(1.0)
}

/// Get known relationships for a book
fn get_known_relationships(
    book_id: &str,
) -> Vec<(&'static str, &'static str, RelationshipType, &'static str)> {
    match book_id {
        "1342" => vec![
            // Pride and Prejudice
            ("Elizabeth Bennet", "Fitzwilliam Darcy", RelationshipType::Romantic, "Central romantic relationship; initially antagonistic, evolving to love and marriage"),
            ("Jane Bennet", "Mr. Bingley", RelationshipType::Romantic, "Secondary romantic relationship; separated by misunderstanding, reunited"),
            ("Elizabeth Bennet", "Jane Bennet", RelationshipType::Family, "Sisters and close confidants"),
            ("Mr. Bennet", "Mrs. Bennet", RelationshipType::Family, "Married couple; contrasting temperaments"),
            ("Lydia Bennet", "Mr. Wickham", RelationshipType::Romantic, "Elopement causing scandal; Wickham is villainous"),
            ("Elizabeth Bennet", "Mr. Wickham", RelationshipType::Other, "Initially friendly; Elizabeth later learns of his true character"),
            ("Fitzwilliam Darcy", "Mr. Wickham", RelationshipType::Antagonist, "Childhood companions turned enemies; Wickham attempted to elope with Darcy's sister"),
            ("Elizabeth Bennet", "Lady Catherine de Bourgh", RelationshipType::Antagonist, "Conflict over Elizabeth's relationship with Darcy"),
            ("Mr. Collins", "Lady Catherine de Bourgh", RelationshipType::Professional, "Mr. Collins is her obsequious clergyman"),
        ],
        "2701" => vec![
            // Moby Dick
            ("Captain Ahab", "Moby Dick", RelationshipType::Antagonist, "Obsessive pursuit; Ahab lost his leg to the whale"),
            ("Ishmael", "Queequeg", RelationshipType::Friend, "Close friendship despite cultural differences"),
            ("Captain Ahab", "Starbuck", RelationshipType::Professional, "Captain and first mate; Starbuck questions Ahab's obsession"),
            ("Ishmael", "Captain Ahab", RelationshipType::Professional, "Narrator observing the captain's descent into madness"),
        ],
        "84" => vec![
            // Frankenstein
            ("Victor Frankenstein", "The Creature", RelationshipType::Other, "Creator and creation; mutual destruction"),
            ("Victor Frankenstein", "Elizabeth Lavenza", RelationshipType::Romantic, "Childhood sweethearts; tragic ending"),
            ("Victor Frankenstein", "Henry Clerval", RelationshipType::Friend, "Close friend; victim of the creature"),
            ("Robert Walton", "Victor Frankenstein", RelationshipType::Other, "Listener to Victor's tale; parallel obsessions"),
        ],
        "1524" => vec![
            // Hamlet
            ("Hamlet", "Claudius", RelationshipType::Antagonist, "Nephew/uncle; Claudius killed Hamlet's father"),
            ("Hamlet", "Gertrude", RelationshipType::Family, "Mother and son; Hamlet resents her remarriage"),
            ("Hamlet", "Ophelia", RelationshipType::Romantic, "Tragic love; Hamlet's behavior contributes to her madness"),
            ("Hamlet", "Horatio", RelationshipType::Friend, "Loyal friend and confidant"),
            ("Polonius", "Ophelia", RelationshipType::Family, "Father and daughter"),
            ("Laertes", "Ophelia", RelationshipType::Family, "Brother and sister"),
            ("Hamlet", "Laertes", RelationshipType::Antagonist, "Final duel; both seek revenge"),
        ],
        "98" => vec![
            // A Tale of Two Cities
            ("Sydney Carton", "Charles Darnay", RelationshipType::Other, "Lookalikes; Carton sacrifices himself for Darnay"),
            ("Charles Darnay", "Lucie Manette", RelationshipType::Romantic, "Marriage despite class differences"),
            ("Sydney Carton", "Lucie Manette", RelationshipType::Romantic, "Unrequited love; his sacrifice is for her"),
            ("Doctor Manette", "Lucie Manette", RelationshipType::Family, "Father and daughter; reunited after imprisonment"),
            ("Madame Defarge", "Charles Darnay", RelationshipType::Antagonist, "Seeks revenge against his family"),
        ],
        _ => vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_known_characters_structure() {
        // Verify all known characters have valid structure
        for (name, aliases, book_ids) in KNOWN_CHARACTERS {
            assert!(!name.is_empty());
            assert!(!aliases.is_empty());
            assert!(!book_ids.is_empty());
        }
    }

    #[test]
    fn test_known_themes_structure() {
        // Verify all known themes have valid structure
        for (name, keywords, description) in KNOWN_THEMES {
            assert!(!name.is_empty());
            assert!(!keywords.is_empty());
            assert!(!description.is_empty());
        }
    }

    #[test]
    fn test_relationship_type_display() {
        assert_eq!(format!("{}", RelationshipType::Romantic), "Romantic");
        assert_eq!(format!("{}", RelationshipType::Family), "Family");
        assert_eq!(format!("{}", RelationshipType::Antagonist), "Antagonist");
    }

    #[test]
    fn test_calculate_theme_relevance() {
        let empty: Vec<EvidenceChunk> = vec![];
        assert!((calculate_theme_relevance(&empty) - 0.0).abs() < f32::EPSILON);

        let evidence = vec![EvidenceChunk {
            content: "test".to_string(),
            book_id: "1".to_string(),
            book_title: "Test".to_string(),
            chunk_index: 0,
            score: 0.8,
        }];
        let score = calculate_theme_relevance(&evidence);
        assert!(score > 0.0);
        assert!(score <= 1.0);
    }

    #[test]
    fn test_known_relationships_pride_and_prejudice() {
        let relationships = get_known_relationships("1342");
        assert!(!relationships.is_empty());

        // Should include Elizabeth and Darcy
        let has_main = relationships.iter().any(|(c1, c2, _, _)| {
            (*c1 == "Elizabeth Bennet" && *c2 == "Fitzwilliam Darcy")
                || (*c1 == "Fitzwilliam Darcy" && *c2 == "Elizabeth Bennet")
        });
        assert!(has_main);
    }

    #[test]
    fn test_known_relationships_unknown_book() {
        let relationships = get_known_relationships("99999");
        assert!(relationships.is_empty());
    }
}
