//! Configuration for Book Search

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Book metadata with additional fields for filtering
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookMetadata {
    /// Gutenberg book ID
    pub id: u32,
    /// Book title
    pub title: String,
    /// Author name
    pub author: String,
    /// Language (ISO 639-1 code: en, fr, de, es, etc.)
    pub language: String,
    /// Genre/category
    pub genre: String,
    /// Publication year (original publication, not Gutenberg upload)
    pub year: Option<i32>,
}

impl BookMetadata {
    /// Create a new BookMetadata with all fields
    pub fn new(
        id: u32,
        title: &str,
        author: &str,
        language: &str,
        genre: &str,
        year: Option<i32>,
    ) -> Self {
        Self {
            id,
            title: title.to_string(),
            author: author.to_string(),
            language: language.to_string(),
            genre: genre.to_string(),
            year,
        }
    }

    /// Create from a simple tuple with default English language and Fiction genre
    pub fn from_tuple(id: u32, title: &str, author: &str) -> Self {
        Self::new(id, title, author, "en", "Fiction", None)
    }
}

/// Available genres for filtering
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Genre {
    Fiction,
    Philosophy,
    Poetry,
    Drama,
    SciFi,
    Mystery,
    Adventure,
    Romance,
    History,
    Science,
    Religion,
    Children,
    Horror,
    Classics,
}

impl Genre {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Fiction => "Fiction",
            Self::Philosophy => "Philosophy",
            Self::Poetry => "Poetry",
            Self::Drama => "Drama",
            Self::SciFi => "Science Fiction",
            Self::Mystery => "Mystery",
            Self::Adventure => "Adventure",
            Self::Romance => "Romance",
            Self::History => "History",
            Self::Science => "Science",
            Self::Religion => "Religion",
            Self::Children => "Children",
            Self::Horror => "Horror",
            Self::Classics => "Classics",
        }
    }
}

/// Book Search configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookSearchConfig {
    /// OpenSearch URL
    pub opensearch_url: String,

    /// OpenSearch index name
    pub index_name: String,

    /// HuggingFace model for embeddings
    pub embedding_model: String,

    /// Embedding dimension (must match model output)
    pub embedding_dim: usize,

    /// Chunk size for text splitting
    pub chunk_size: usize,

    /// Chunk overlap
    pub chunk_overlap: usize,

    /// Local cache directory for downloaded books
    pub cache_dir: PathBuf,

    /// Prometheus metrics port
    pub metrics_port: u16,

    /// OTLP endpoint for tracing
    pub otlp_endpoint: Option<String>,
}

impl Default for BookSearchConfig {
    fn default() -> Self {
        Self {
            opensearch_url: "http://localhost:9200".to_string(),
            index_name: "books".to_string(),
            embedding_model: "sentence-transformers/all-MiniLM-L6-v2".to_string(),
            embedding_dim: 384,
            chunk_size: 1000,
            chunk_overlap: 200,
            cache_dir: PathBuf::from("data/gutenberg"),
            metrics_port: 9091,
            otlp_endpoint: Some("http://localhost:4317".to_string()),
        }
    }
}

/// Preset book collections
#[derive(Debug, Clone, Copy)]
pub enum BookPreset {
    /// 10 famous books (~5MB, ~5K chunks)
    Quick,
    /// 50 classic books (~25MB, ~25K chunks)
    Classics,
    /// 100+ classic books (~60MB, ~60K chunks)
    Full,
    /// 1000+ English books (~500MB, ~500K chunks)
    Massive,
    /// Books in original non-English languages (French, German, Spanish, Italian, Latin)
    Multilingual,
    /// ALL available English text books from Project Gutenberg (~70,000 books)
    /// Uses dynamic catalog discovery via GutenbergCatalog
    Gutenberg,
}

impl BookPreset {
    /// Get book IDs and metadata for this preset
    pub fn books(&self) -> Vec<(u32, &'static str, &'static str)> {
        match self {
            Self::Quick => vec![
                (1342, "Pride and Prejudice", "Jane Austen"),
                (2701, "Moby Dick", "Herman Melville"),
                (11, "Alice's Adventures in Wonderland", "Lewis Carroll"),
                (84, "Frankenstein", "Mary Shelley"),
                (
                    1661,
                    "The Adventures of Sherlock Holmes",
                    "Arthur Conan Doyle",
                ),
                (98, "A Tale of Two Cities", "Charles Dickens"),
                (74, "The Adventures of Tom Sawyer", "Mark Twain"),
                (1232, "The Prince", "Niccolò Machiavelli"),
                (345, "Dracula", "Bram Stoker"),
                (2600, "War and Peace", "Leo Tolstoy"),
            ],
            Self::Classics => {
                let mut books = Self::Quick.books();
                books.extend(vec![
                    (1080, "A Modest Proposal", "Jonathan Swift"),
                    (16328, "Beowulf", "Anonymous"),
                    (768, "Wuthering Heights", "Emily Brontë"),
                    (1400, "Great Expectations", "Charles Dickens"),
                    (174, "The Picture of Dorian Gray", "Oscar Wilde"),
                    (120, "Treasure Island", "Robert Louis Stevenson"),
                    (219, "Heart of Darkness", "Joseph Conrad"),
                    (1260, "Jane Eyre", "Charlotte Brontë"),
                    (5200, "Metamorphosis", "Franz Kafka"),
                    (244, "A Study in Scarlet", "Arthur Conan Doyle"),
                    (1952, "The Yellow Wallpaper", "Charlotte Perkins Gilman"),
                    (76, "Adventures of Huckleberry Finn", "Mark Twain"),
                    (55, "The Wonderful Wizard of Oz", "L. Frank Baum"),
                    (1184, "The Count of Monte Cristo", "Alexandre Dumas"),
                    (4300, "Ulysses", "James Joyce"),
                    (28054, "The Brothers Karamazov", "Fyodor Dostoevsky"),
                    (2554, "Crime and Punishment", "Fyodor Dostoevsky"),
                    (36, "The War of the Worlds", "H.G. Wells"),
                    (35, "The Time Machine", "H.G. Wells"),
                    (1934, "Songs of Innocence and Experience", "William Blake"),
                    (158, "Emma", "Jane Austen"),
                    (161, "Sense and Sensibility", "Jane Austen"),
                    (105, "Persuasion", "Jane Austen"),
                    (145, "Middlemarch", "George Eliot"),
                    (1727, "The Odyssey", "Homer"),
                    (6130, "The Iliad", "Homer"),
                    (1497, "The Republic", "Plato"),
                    (2009, "The Origin of Species", "Charles Darwin"),
                    (4363, "Beyond Good and Evil", "Friedrich Nietzsche"),
                    (132, "The Art of War", "Sun Tzu"),
                    (996, "Don Quixote", "Miguel de Cervantes"),
                    (1399, "Anna Karenina", "Leo Tolstoy"),
                    (25344, "The Scarlet Letter", "Nathaniel Hawthorne"),
                    (209, "The Turn of the Screw", "Henry James"),
                    (113, "The Secret Garden", "Frances Hodgson Burnett"),
                    (236, "The Jungle Book", "Rudyard Kipling"),
                    (1322, "Leaves of Grass", "Walt Whitman"),
                    (100, "Complete Works of Shakespeare", "William Shakespeare"),
                    (1251, "Le Morte d'Arthur", "Thomas Malory"),
                    (3600, "Thus Spoke Zarathustra", "Friedrich Nietzsche"),
                ]);
                books
            }
            Self::Full => {
                let mut books = Self::Classics.books();
                books.extend(vec![
                    // More Victorian Literature
                    (730, "Oliver Twist", "Charles Dickens"),
                    (766, "David Copperfield", "Charles Dickens"),
                    (786, "Hard Times", "Charles Dickens"),
                    (1023, "Bleak House", "Charles Dickens"),
                    (883, "Little Dorrit", "Charles Dickens"),
                    (564, "Our Mutual Friend", "Charles Dickens"),
                    (580, "The Pickwick Papers", "Charles Dickens"),
                    (917, "The Old Curiosity Shop", "Charles Dickens"),
                    (821, "Dombey and Son", "Charles Dickens"),
                    (653, "Nicholas Nickleby", "Charles Dickens"),
                    // More Jane Austen
                    (121, "Northanger Abbey", "Jane Austen"),
                    (1212, "Lady Susan", "Jane Austen"),
                    (946, "Mansfield Park", "Jane Austen"),
                    // More American Literature
                    (45, "Anne of Green Gables", "L. M. Montgomery"),
                    (514, "Little Women", "Louisa May Alcott"),
                    (2147, "The Pit and the Pendulum", "Edgar Allan Poe"),
                    (932, "The Fall of the House of Usher", "Edgar Allan Poe"),
                    (2148, "The Raven", "Edgar Allan Poe"),
                    (2151, "The Masque of the Red Death", "Edgar Allan Poe"),
                    (
                        73,
                        "Autobiography of Benjamin Franklin",
                        "Benjamin Franklin",
                    ),
                    (30, "Walden", "Henry David Thoreau"),
                    (7849, "Civil Disobedience", "Henry David Thoreau"),
                    (62, "A Princess of Mars", "Edgar Rice Burroughs"),
                    (78, "Tarzan of the Apes", "Edgar Rice Burroughs"),
                    (2500, "Siddhartha", "Hermann Hesse"),
                    // British Classics
                    (2814, "Dubliners", "James Joyce"),
                    (4217, "A Portrait of the Artist", "James Joyce"),
                    (5230, "The Invisible Man", "H.G. Wells"),
                    (5233, "The Island of Dr Moreau", "H.G. Wells"),
                    (159, "The Son of Tarzan", "Edgar Rice Burroughs"),
                    (27780, "Nostromo", "Joseph Conrad"),
                    (526, "Kim", "Rudyard Kipling"),
                    (1937, "The Man Who Would Be King", "Rudyard Kipling"),
                    (6053, "Peter Pan", "J. M. Barrie"),
                    (1998, "The Wind in the Willows", "Kenneth Grahame"),
                    (1, "The Declaration of Independence", "Thomas Jefferson"),
                    // Philosophy and Essays
                    (10615, "Leviathan", "Thomas Hobbes"),
                    (5827, "Second Treatise on Government", "John Locke"),
                    (3207, "Utopia", "Thomas More"),
                    (
                        7370,
                        "An Enquiry Concerning Human Understanding",
                        "David Hume",
                    ),
                    (5740, "Critique of Pure Reason", "Immanuel Kant"),
                    (8438, "The Social Contract", "Jean-Jacques Rousseau"),
                    (38427, "The Communist Manifesto", "Marx and Engels"),
                    // Russian Literature
                    (600, "Notes from Underground", "Fyodor Dostoevsky"),
                    (2892, "The Gambler", "Fyodor Dostoevsky"),
                    (2197, "The Idiot", "Fyodor Dostoevsky"),
                    (689, "The Death of Ivan Ilyich", "Leo Tolstoy"),
                    (986, "A Confession", "Leo Tolstoy"),
                    (985, "The Kreutzer Sonata", "Leo Tolstoy"),
                    // French Literature
                    (135, "Les Misérables", "Victor Hugo"),
                    (2610, "The Hunchback of Notre Dame", "Victor Hugo"),
                    (60, "The Scarlet Pimpernel", "Baroness Orczy"),
                    (8117, "Candide", "Voltaire"),
                    (5423, "Germinal", "Émile Zola"),
                    // Science and Natural History
                    (1228, "On the Genealogy of Morals", "Friedrich Nietzsche"),
                    (36034, "The Voyage of the Beagle", "Charles Darwin"),
                    (2300, "Descent of Man", "Charles Darwin"),
                    (2034, "The Expression of Emotions", "Charles Darwin"),
                    // Adventure and Travel
                    (164, "Twenty Thousand Leagues Under the Sea", "Jules Verne"),
                    (83, "Around the World in Eighty Days", "Jules Verne"),
                    (103, "Journey to the Center of the Earth", "Jules Verne"),
                    (1268, "The Mysterious Island", "Jules Verne"),
                    (18857, "From the Earth to the Moon", "Jules Verne"),
                    (8799, "Robinson Crusoe", "Daniel Defoe"),
                    (5343, "Gulliver's Travels", "Jonathan Swift"),
                    // Gothic and Horror
                    (42324, "The Castle of Otranto", "Horace Walpole"),
                    (696, "The Monk", "Matthew Lewis"),
                    (
                        43,
                        "The Strange Case of Dr Jekyll and Mr Hyde",
                        "Robert Louis Stevenson",
                    ),
                    (19033, "Carmilla", "Sheridan Le Fanu"),
                    (1695, "The Legend of Sleepy Hollow", "Washington Irving"),
                    // Poetry Collections
                    (1065, "Paradise Lost", "John Milton"),
                    (22120, "The Divine Comedy", "Dante Alighieri"),
                    (4705, "The Canterbury Tales", "Geoffrey Chaucer"),
                    (10217, "Poems", "Emily Dickinson"),
                    (8606, "Selected Poems", "John Keats"),
                    // Plays
                    (5053, "A Doll's House", "Henrik Ibsen"),
                    (2162, "An Enemy of the People", "Henrik Ibsen"),
                    (1338, "The Importance of Being Earnest", "Oscar Wilde"),
                    (5629, "Pygmalion", "George Bernard Shaw"),
                    (844, "The Merchant of Venice", "William Shakespeare"),
                    // Mystery and Detective
                    (1155, "The Sign of the Four", "Arthur Conan Doyle"),
                    (2852, "The Hound of the Baskervilles", "Arthur Conan Doyle"),
                    (221, "The Valley of Fear", "Arthur Conan Doyle"),
                    (2097, "The Return of Sherlock Holmes", "Arthur Conan Doyle"),
                    (2343, "The Memoirs of Sherlock Holmes", "Arthur Conan Doyle"),
                ]);
                books
            }
            Self::Massive => {
                let mut books = Self::Full.books();
                books.extend(vec![
                    // ═══════════════════════════════════════════════════════════════
                    // SHAKESPEARE - Individual Plays and Poems (~40 works)
                    // ═══════════════════════════════════════════════════════════════
                    (1524, "Hamlet", "William Shakespeare"),
                    (1533, "Macbeth", "William Shakespeare"),
                    (1519, "King Lear", "William Shakespeare"),
                    (1531, "Othello", "William Shakespeare"),
                    (1500, "A Midsummer Night's Dream", "William Shakespeare"),
                    (1508, "Much Ado About Nothing", "William Shakespeare"),
                    (1522, "The Tempest", "William Shakespeare"),
                    (1502, "As You Like It", "William Shakespeare"),
                    (1526, "Twelfth Night", "William Shakespeare"),
                    (1532, "The Comedy of Errors", "William Shakespeare"),
                    (1539, "Julius Caesar", "William Shakespeare"),
                    (1540, "Antony and Cleopatra", "William Shakespeare"),
                    (1541, "Coriolanus", "William Shakespeare"),
                    (1542, "Timon of Athens", "William Shakespeare"),
                    (1505, "King John", "William Shakespeare"),
                    (1503, "Richard II", "William Shakespeare"),
                    (1504, "Richard III", "William Shakespeare"),
                    (1511, "Henry IV Part 1", "William Shakespeare"),
                    (1512, "Henry IV Part 2", "William Shakespeare"),
                    (1515, "Henry V", "William Shakespeare"),
                    (1517, "Henry VI Part 1", "William Shakespeare"),
                    (1518, "Henry VI Part 2", "William Shakespeare"),
                    (1520, "Henry VI Part 3", "William Shakespeare"),
                    (1528, "Henry VIII", "William Shakespeare"),
                    (1529, "Titus Andronicus", "William Shakespeare"),
                    (1530, "Troilus and Cressida", "William Shakespeare"),
                    (1534, "Cymbeline", "William Shakespeare"),
                    (1535, "Pericles", "William Shakespeare"),
                    (1536, "The Winter's Tale", "William Shakespeare"),
                    (1537, "The Two Gentlemen of Verona", "William Shakespeare"),
                    (1538, "The Merry Wives of Windsor", "William Shakespeare"),
                    (1507, "Love's Labour's Lost", "William Shakespeare"),
                    (1525, "The Taming of the Shrew", "William Shakespeare"),
                    (1514, "Measure for Measure", "William Shakespeare"),
                    (1509, "All's Well That Ends Well", "William Shakespeare"),
                    (1790, "Sonnets", "William Shakespeare"),
                    (1538, "Venus and Adonis", "William Shakespeare"),
                    // ═══════════════════════════════════════════════════════════════
                    // CHARLES DICKENS - Complete Works (~30 more)
                    // ═══════════════════════════════════════════════════════════════
                    (46, "A Christmas Carol", "Charles Dickens"),
                    (644, "Martin Chuzzlewit", "Charles Dickens"),
                    (824, "The Mystery of Edwin Drood", "Charles Dickens"),
                    (588, "Barnaby Rudge", "Charles Dickens"),
                    (912, "American Notes", "Charles Dickens"),
                    (700, "Sketches by Boz", "Charles Dickens"),
                    (1467, "Master Humphrey's Clock", "Charles Dickens"),
                    (30368, "Pictures from Italy", "Charles Dickens"),
                    (675, "The Uncommercial Traveller", "Charles Dickens"),
                    (710, "A Child's History of England", "Charles Dickens"),
                    (882, "Reprinted Pieces", "Charles Dickens"),
                    (1023, "Bleak House", "Charles Dickens"),
                    (564, "Our Mutual Friend", "Charles Dickens"),
                    (
                        968,
                        "The Haunted Man and Ghost's Bargain",
                        "Charles Dickens",
                    ),
                    (678, "The Battle of Life", "Charles Dickens"),
                    (676, "The Cricket on the Hearth", "Charles Dickens"),
                    (677, "The Chimes", "Charles Dickens"),
                    (
                        914,
                        "The Lazy Tour of Two Idle Apprentices",
                        "Charles Dickens",
                    ),
                    (924, "Mudfog and Other Sketches", "Charles Dickens"),
                    (1407, "A Message from the Sea", "Charles Dickens"),
                    // ═══════════════════════════════════════════════════════════════
                    // MARK TWAIN - Complete Works (~35 more)
                    // ═══════════════════════════════════════════════════════════════
                    (86, "The Prince and the Pauper", "Mark Twain"),
                    (1837, "Life on the Mississippi", "Mark Twain"),
                    (70, "The Innocents Abroad", "Mark Twain"),
                    (119, "A Tramp Abroad", "Mark Twain"),
                    (
                        91,
                        "A Connecticut Yankee in King Arthur's Court",
                        "Mark Twain",
                    ),
                    (102, "The Tragedy of Pudd'nhead Wilson", "Mark Twain"),
                    (3176, "Personal Recollections of Joan of Arc", "Mark Twain"),
                    (142, "The Gilded Age", "Mark Twain"),
                    (3177, "The American Claimant", "Mark Twain"),
                    (3183, "Tom Sawyer Abroad", "Mark Twain"),
                    (93, "Tom Sawyer, Detective", "Mark Twain"),
                    (3185, "Those Extraordinary Twins", "Mark Twain"),
                    (3192, "The Mysterious Stranger", "Mark Twain"),
                    (3191, "What is Man? and Other Essays", "Mark Twain"),
                    (3186, "Christian Science", "Mark Twain"),
                    (3178, "The Man That Corrupted Hadleyburg", "Mark Twain"),
                    (3195, "Following the Equator", "Mark Twain"),
                    (3189, "Roughing It", "Mark Twain"),
                    (245, "Autobiography of Mark Twain", "Mark Twain"),
                    (1044, "Letters from the Earth", "Mark Twain"),
                    (3188, "A Double Barrelled Detective Story", "Mark Twain"),
                    (50740, "Eve's Diary", "Mark Twain"),
                    (19987, "Extracts from Adam's Diary", "Mark Twain"),
                    (7100, "A Horse's Tale", "Mark Twain"),
                    (7244, "The £1,000,000 Bank-Note", "Mark Twain"),
                    // ═══════════════════════════════════════════════════════════════
                    // JACK LONDON - Complete Works (~40 more)
                    // ═══════════════════════════════════════════════════════════════
                    (215, "The Call of the Wild", "Jack London"),
                    (910, "White Fang", "Jack London"),
                    (1164, "The Sea-Wolf", "Jack London"),
                    (2400, "Martin Eden", "Jack London"),
                    (56, "The Iron Heel", "Jack London"),
                    (140, "The Scarlet Plague", "Jack London"),
                    (1133, "The People of the Abyss", "Jack London"),
                    (587, "South Sea Tales", "Jack London"),
                    (14658, "Tales of the Fish Patrol", "Jack London"),
                    (1200, "Burning Daylight", "Jack London"),
                    (5737, "The Road", "Jack London"),
                    (2782, "John Barleycorn", "Jack London"),
                    (14859, "Smoke Bellew", "Jack London"),
                    (25164, "Jerry of the Islands", "Jack London"),
                    (25192, "Michael, Brother of Jerry", "Jack London"),
                    (21970, "The Star Rover", "Jack London"),
                    (1083, "Before Adam", "Jack London"),
                    (21700, "Adventure", "Jack London"),
                    (2618, "A Daughter of the Snows", "Jack London"),
                    (151, "The Son of the Wolf", "Jack London"),
                    (152, "Children of the Frost", "Jack London"),
                    (167, "The Faith of Men", "Jack London"),
                    (223, "Lost Face", "Jack London"),
                    (14624, "Moon-Face and Other Stories", "Jack London"),
                    (22696, "When God Laughs", "Jack London"),
                    (13736, "Love of Life and Other Stories", "Jack London"),
                    (1083, "The House of Pride", "Jack London"),
                    (5814, "The Game", "Jack London"),
                    (1050, "The Valley of the Moon", "Jack London"),
                    (14567, "The Little Lady of the Big House", "Jack London"),
                    // ═══════════════════════════════════════════════════════════════
                    // EDGAR ALLAN POE - Complete Works (~30 more)
                    // ═══════════════════════════════════════════════════════════════
                    (2149, "The Cask of Amontillado", "Edgar Allan Poe"),
                    (2150, "The Tell-Tale Heart", "Edgar Allan Poe"),
                    (2152, "The Murders in the Rue Morgue", "Edgar Allan Poe"),
                    (2153, "The Gold-Bug", "Edgar Allan Poe"),
                    (2155, "The Black Cat", "Edgar Allan Poe"),
                    (2156, "Ligeia", "Edgar Allan Poe"),
                    (2157, "Eleonora", "Edgar Allan Poe"),
                    (
                        50852,
                        "The Narrative of Arthur Gordon Pym",
                        "Edgar Allan Poe",
                    ),
                    (932, "Tales of Mystery and Imagination", "Edgar Allan Poe"),
                    (
                        10031,
                        "The Works of Edgar Allan Poe Vol 1",
                        "Edgar Allan Poe",
                    ),
                    (
                        10032,
                        "The Works of Edgar Allan Poe Vol 2",
                        "Edgar Allan Poe",
                    ),
                    (
                        10033,
                        "The Works of Edgar Allan Poe Vol 3",
                        "Edgar Allan Poe",
                    ),
                    (
                        10034,
                        "The Works of Edgar Allan Poe Vol 4",
                        "Edgar Allan Poe",
                    ),
                    (
                        10035,
                        "The Works of Edgar Allan Poe Vol 5",
                        "Edgar Allan Poe",
                    ),
                    (25525, "The Raven and Other Poems", "Edgar Allan Poe"),
                    // ═══════════════════════════════════════════════════════════════
                    // THOMAS HARDY - Complete Works (~25 more)
                    // ═══════════════════════════════════════════════════════════════
                    (110, "Tess of the d'Urbervilles", "Thomas Hardy"),
                    (153, "Jude the Obscure", "Thomas Hardy"),
                    (107, "The Return of the Native", "Thomas Hardy"),
                    (108, "Far from the Madding Crowd", "Thomas Hardy"),
                    (3056, "The Mayor of Casterbridge", "Thomas Hardy"),
                    (28239, "A Pair of Blue Eyes", "Thomas Hardy"),
                    (5765, "Two on a Tower", "Thomas Hardy"),
                    (3038, "Under the Greenwood Tree", "Thomas Hardy"),
                    (1619, "The Trumpet-Major", "Thomas Hardy"),
                    (3042, "The Hand of Ethelberta", "Thomas Hardy"),
                    (9260, "The Woodlanders", "Thomas Hardy"),
                    (5760, "A Laodicean", "Thomas Hardy"),
                    (3040, "Desperate Remedies", "Thomas Hardy"),
                    (6320, "The Well-Beloved", "Thomas Hardy"),
                    (3044, "Life's Little Ironies", "Thomas Hardy"),
                    (5772, "Wessex Tales", "Thomas Hardy"),
                    (271, "A Changed Man", "Thomas Hardy"),
                    (467, "A Group of Noble Dames", "Thomas Hardy"),
                    // ═══════════════════════════════════════════════════════════════
                    // ANTHONY TROLLOPE - Complete Works (~50 more)
                    // ═══════════════════════════════════════════════════════════════
                    (815, "Barchester Towers", "Anthony Trollope"),
                    (6439, "Doctor Thorne", "Anthony Trollope"),
                    (9951, "Framley Parsonage", "Anthony Trollope"),
                    (11208, "The Small House at Allington", "Anthony Trollope"),
                    (16001, "The Last Chronicle of Barset", "Anthony Trollope"),
                    (2085, "The Warden", "Anthony Trollope"),
                    (19664, "Can You Forgive Her?", "Anthony Trollope"),
                    (20239, "Phineas Finn", "Anthony Trollope"),
                    (12115, "The Eustace Diamonds", "Anthony Trollope"),
                    (18640, "Phineas Redux", "Anthony Trollope"),
                    (13179, "The Prime Minister", "Anthony Trollope"),
                    (12011, "The Duke's Children", "Anthony Trollope"),
                    (9868, "The Way We Live Now", "Anthony Trollope"),
                    (13277, "An Autobiography", "Anthony Trollope"),
                    (15405, "Orley Farm", "Anthony Trollope"),
                    (17759, "He Knew He Was Right", "Anthony Trollope"),
                    (17232, "The Vicar of Bullhampton", "Anthony Trollope"),
                    (
                        17474,
                        "Sir Harry Hotspur of Humblethwaite",
                        "Anthony Trollope",
                    ),
                    (6660, "Ralph the Heir", "Anthony Trollope"),
                    (18773, "Lady Anna", "Anthony Trollope"),
                    (15017, "Is He Popenjoy?", "Anthony Trollope"),
                    (14666, "The American Senator", "Anthony Trollope"),
                    (16011, "Ayala's Angel", "Anthony Trollope"),
                    (17883, "Marion Fay", "Anthony Trollope"),
                    (18050, "Kept in the Dark", "Anthony Trollope"),
                    (17888, "An Old Man's Love", "Anthony Trollope"),
                    (17989, "The Landleaguers", "Anthony Trollope"),
                    (6444, "Castle Richmond", "Anthony Trollope"),
                    (6661, "The Kellys and the O'Kellys", "Anthony Trollope"),
                    (6665, "La Vendée", "Anthony Trollope"),
                    (17765, "The Three Clerks", "Anthony Trollope"),
                    (17886, "The Bertrams", "Anthony Trollope"),
                    (15860, "Miss Mackenzie", "Anthony Trollope"),
                    (18771, "The Belton Estate", "Anthony Trollope"),
                    (17778, "Nina Balatka", "Anthony Trollope"),
                    (17777, "Linda Tressel", "Anthony Trollope"),
                    (16753, "The Claverings", "Anthony Trollope"),
                    (17779, "The Golden Lion of Granpere", "Anthony Trollope"),
                    (13653, "Harry Heathcote of Gangoil", "Anthony Trollope"),
                    // ═══════════════════════════════════════════════════════════════
                    // H.G. WELLS - Complete Works (~30 more)
                    // ═══════════════════════════════════════════════════════════════
                    (159, "The First Men in the Moon", "H.G. Wells"),
                    (718, "The Food of the Gods", "H.G. Wells"),
                    (1743, "A Modern Utopia", "H.G. Wells"),
                    (1958, "In the Days of the Comet", "H.G. Wells"),
                    (27706, "The Sleeper Awakes", "H.G. Wells"),
                    (4059, "Ann Veronica", "H.G. Wells"),
                    (6927, "The History of Mr Polly", "H.G. Wells"),
                    (1672, "Kipps", "H.G. Wells"),
                    (14965, "Tono-Bungay", "H.G. Wells"),
                    (3643, "The New Machiavelli", "H.G. Wells"),
                    (1869, "The Wonderful Visit", "H.G. Wells"),
                    (11866, "The Wheels of Chance", "H.G. Wells"),
                    (1569, "Love and Mr Lewisham", "H.G. Wells"),
                    (11870, "The Soul of a Bishop", "H.G. Wells"),
                    (12750, "Joan and Peter", "H.G. Wells"),
                    (6736, "The World Set Free", "H.G. Wells"),
                    (7058, "The Dream", "H.G. Wells"),
                    (13174, "The Secret Places of the Heart", "H.G. Wells"),
                    (24965, "Men Like Gods", "H.G. Wells"),
                    (718, "The Plattner Story", "H.G. Wells"),
                    (5311, "Tales of Space and Time", "H.G. Wells"),
                    (1218, "Twelve Stories and a Dream", "H.G. Wells"),
                    (6929, "The Country of the Blind", "H.G. Wells"),
                    (6739, "The Door in the Wall", "H.G. Wells"),
                    (26922, "Short Stories", "H.G. Wells"),
                    // ═══════════════════════════════════════════════════════════════
                    // ARTHUR CONAN DOYLE - Complete Works (~40 more)
                    // ═══════════════════════════════════════════════════════════════
                    (108, "His Last Bow", "Arthur Conan Doyle"),
                    (
                        2350,
                        "The Case-Book of Sherlock Holmes",
                        "Arthur Conan Doyle",
                    ),
                    (2344, "A Study in Scarlet", "Arthur Conan Doyle"),
                    (863, "The Lost World", "Arthur Conan Doyle"),
                    (139, "The Poison Belt", "Arthur Conan Doyle"),
                    (2488, "The Land of Mist", "Arthur Conan Doyle"),
                    (5322, "The White Company", "Arthur Conan Doyle"),
                    (903, "Sir Nigel", "Arthur Conan Doyle"),
                    (834, "Micah Clarke", "Arthur Conan Doyle"),
                    (10446, "The Refugees", "Arthur Conan Doyle"),
                    (
                        5148,
                        "The Exploits of Brigadier Gerard",
                        "Arthur Conan Doyle",
                    ),
                    (11656, "The Adventures of Gerard", "Arthur Conan Doyle"),
                    (294, "Rodney Stone", "Arthur Conan Doyle"),
                    (5258, "The Tragedy of the Korosko", "Arthur Conan Doyle"),
                    (8394, "Beyond the City", "Arthur Conan Doyle"),
                    (5317, "The Great Shadow", "Arthur Conan Doyle"),
                    (10581, "Uncle Bernac", "Arthur Conan Doyle"),
                    (
                        7452,
                        "A Duet, with an Occasional Chorus",
                        "Arthur Conan Doyle",
                    ),
                    (8727, "The Firm of Girdlestone", "Arthur Conan Doyle"),
                    (834, "The Stark Munro Letters", "Arthur Conan Doyle"),
                    (5323, "Round the Red Lamp", "Arthur Conan Doyle"),
                    (4295, "The Doings of Raffles Haw", "Arthur Conan Doyle"),
                    (3289, "Tales of Terror and Mystery", "Arthur Conan Doyle"),
                    (5260, "The Captain of the Polestar", "Arthur Conan Doyle"),
                    (26153, "Danger! and Other Stories", "Arthur Conan Doyle"),
                    (58774, "The Horror of the Heights", "Arthur Conan Doyle"),
                    // ═══════════════════════════════════════════════════════════════
                    // NATHANIEL HAWTHORNE - Complete Works (~20 more)
                    // ═══════════════════════════════════════════════════════════════
                    (77, "The House of the Seven Gables", "Nathaniel Hawthorne"),
                    (2181, "The Blithedale Romance", "Nathaniel Hawthorne"),
                    (2182, "The Marble Faun", "Nathaniel Hawthorne"),
                    (8095, "Mosses from an Old Manse", "Nathaniel Hawthorne"),
                    (9209, "Twice Told Tales", "Nathaniel Hawthorne"),
                    (9212, "The Snow-Image", "Nathaniel Hawthorne"),
                    (
                        7140,
                        "A Wonder Book for Girls and Boys",
                        "Nathaniel Hawthorne",
                    ),
                    (15166, "Tanglewood Tales", "Nathaniel Hawthorne"),
                    (512, "Fanshawe", "Nathaniel Hawthorne"),
                    (8084, "Septimius Felton", "Nathaniel Hawthorne"),
                    (8083, "The Dolliver Romance", "Nathaniel Hawthorne"),
                    (8086, "Doctor Grimshawe's Secret", "Nathaniel Hawthorne"),
                    (13775, "The Ancestral Footstep", "Nathaniel Hawthorne"),
                    (
                        9211,
                        "Passages from American Note-Books",
                        "Nathaniel Hawthorne",
                    ),
                    (
                        9213,
                        "Passages from English Note-Books",
                        "Nathaniel Hawthorne",
                    ),
                    (
                        8090,
                        "Passages from French and Italian Note-Books",
                        "Nathaniel Hawthorne",
                    ),
                    // ═══════════════════════════════════════════════════════════════
                    // HENRY JAMES - Complete Works (~30 more)
                    // ═══════════════════════════════════════════════════════════════
                    (432, "The Portrait of a Lady", "Henry James"),
                    (639, "The Ambassadors", "Henry James"),
                    (176, "The Wings of the Dove", "Henry James"),
                    (6893, "The Golden Bowl", "Henry James"),
                    (177, "The American", "Henry James"),
                    (212, "Washington Square", "Henry James"),
                    (175, "The Europeans", "Henry James"),
                    (1075, "Daisy Miller", "Henry James"),
                    (178, "Roderick Hudson", "Henry James"),
                    (211, "The Bostonians", "Henry James"),
                    (1030, "The Princess Casamassima", "Henry James"),
                    (6889, "The Tragic Muse", "Henry James"),
                    (9054, "The Spoils of Poynton", "Henry James"),
                    (490, "What Maisie Knew", "Henry James"),
                    (6898, "The Awkward Age", "Henry James"),
                    (6896, "The Sacred Fount", "Henry James"),
                    (6894, "The Other House", "Henry James"),
                    (6892, "The Outcry", "Henry James"),
                    (1507, "The Reverberator", "Henry James"),
                    (1182, "The Aspern Papers", "Henry James"),
                    (1137, "The Figure in the Carpet", "Henry James"),
                    (1184, "In the Cage", "Henry James"),
                    (10280, "The Beast in the Jungle", "Henry James"),
                    (9100, "The Jolly Corner", "Henry James"),
                    // ═══════════════════════════════════════════════════════════════
                    // GEORGE ELIOT - Complete Works (~15 more)
                    // ═══════════════════════════════════════════════════════════════
                    (507, "Silas Marner", "George Eliot"),
                    (6688, "Adam Bede", "George Eliot"),
                    (550, "The Mill on the Floss", "George Eliot"),
                    (3047, "Daniel Deronda", "George Eliot"),
                    (550, "Romola", "George Eliot"),
                    (17768, "Felix Holt, the Radical", "George Eliot"),
                    (6662, "Scenes of Clerical Life", "George Eliot"),
                    (6676, "Impressions of Theophrastus Such", "George Eliot"),
                    (20618, "Brother Jacob", "George Eliot"),
                    (17370, "The Lifted Veil", "George Eliot"),
                    // ═══════════════════════════════════════════════════════════════
                    // JOSEPH CONRAD - Complete Works (~25 more)
                    // ═══════════════════════════════════════════════════════════════
                    (974, "Lord Jim", "Joseph Conrad"),
                    (526, "The Secret Agent", "Joseph Conrad"),
                    (2021, "Under Western Eyes", "Joseph Conrad"),
                    (876, "Victory", "Joseph Conrad"),
                    (5308, "Chance", "Joseph Conrad"),
                    (1129, "An Outcast of the Islands", "Joseph Conrad"),
                    (125, "Almayer's Folly", "Joseph Conrad"),
                    (1772, "The Arrow of Gold", "Joseph Conrad"),
                    (2021, "The Rescue", "Joseph Conrad"),
                    (331, "The Rover", "Joseph Conrad"),
                    (12091, "Suspense", "Joseph Conrad"),
                    (799, "Typhoon", "Joseph Conrad"),
                    (502, "Youth", "Joseph Conrad"),
                    (526, "Falk", "Joseph Conrad"),
                    (491, "Amy Foster", "Joseph Conrad"),
                    (1202, "Tomorrow", "Joseph Conrad"),
                    (1695, "The End of the Tether", "Joseph Conrad"),
                    (1580, "The Nigger of the Narcissus", "Joseph Conrad"),
                    (1781, "Tales of Unrest", "Joseph Conrad"),
                    (1785, "A Set of Six", "Joseph Conrad"),
                    (1775, "'Twixt Land and Sea", "Joseph Conrad"),
                    (8832, "Within the Tides", "Joseph Conrad"),
                    (8798, "Tales of Hearsay", "Joseph Conrad"),
                    // ═══════════════════════════════════════════════════════════════
                    // ALEXANDRE DUMAS - Complete Works (~30 more)
                    // ═══════════════════════════════════════════════════════════════
                    (1257, "The Three Musketeers", "Alexandre Dumas"),
                    (1258, "Twenty Years After", "Alexandre Dumas"),
                    (1259, "The Vicomte de Bragelonne", "Alexandre Dumas"),
                    (2759, "Louise de la Vallière", "Alexandre Dumas"),
                    (2609, "The Man in the Iron Mask", "Alexandre Dumas"),
                    (126, "The Count of Monte Cristo", "Alexandre Dumas"),
                    (965, "The Black Tulip", "Alexandre Dumas"),
                    (1262, "The Forty-Five Guardsmen", "Alexandre Dumas"),
                    (12461, "Chicot the Jester", "Alexandre Dumas"),
                    (12458, "Marguerite de Valois", "Alexandre Dumas"),
                    (13652, "The Two Dianas", "Alexandre Dumas"),
                    (13721, "The Page of the Duke of Savoy", "Alexandre Dumas"),
                    (17148, "The War of Women", "Alexandre Dumas"),
                    (17149, "The Regent's Daughter", "Alexandre Dumas"),
                    (35546, "The Chevalier d'Harmental", "Alexandre Dumas"),
                    (35562, "Olympe de Clèves", "Alexandre Dumas"),
                    (42612, "The Conspirators", "Alexandre Dumas"),
                    (42611, "The Knight of Maison-Rouge", "Alexandre Dumas"),
                    (11953, "The Corsican Brothers", "Alexandre Dumas"),
                    (965, "Camille", "Alexandre Dumas"),
                    (7737, "Joseph Balsamo", "Alexandre Dumas"),
                    (7738, "The Countess de Charny", "Alexandre Dumas"),
                    (7784, "The Queen's Necklace", "Alexandre Dumas"),
                    (7736, "Ange Pitou", "Alexandre Dumas"),
                    // ═══════════════════════════════════════════════════════════════
                    // VICTOR HUGO - Complete Works (~15 more)
                    // ═══════════════════════════════════════════════════════════════
                    (17489, "The Man Who Laughs", "Victor Hugo"),
                    (6815, "The Toilers of the Sea", "Victor Hugo"),
                    (17147, "Ninety-Three", "Victor Hugo"),
                    (9640, "Hans of Iceland", "Victor Hugo"),
                    (1452, "Bug-Jargal", "Victor Hugo"),
                    (32822, "The Last Day of a Condemned Man", "Victor Hugo"),
                    (37134, "Claude Gueux", "Victor Hugo"),
                    (10209, "William Shakespeare", "Victor Hugo"),
                    (26024, "Napoleon the Little", "Victor Hugo"),
                    (9622, "History of a Crime", "Victor Hugo"),
                    // ═══════════════════════════════════════════════════════════════
                    // WALTER SCOTT - Complete Works (~30 more)
                    // ═══════════════════════════════════════════════════════════════
                    (82, "Ivanhoe", "Walter Scott"),
                    (6926, "Rob Roy", "Walter Scott"),
                    (1463, "Waverley", "Walter Scott"),
                    (1426, "Kenilworth", "Walter Scott"),
                    (1454, "Old Mortality", "Walter Scott"),
                    (1455, "The Heart of Midlothian", "Walter Scott"),
                    (1456, "The Bride of Lammermoor", "Walter Scott"),
                    (1459, "A Legend of Montrose", "Walter Scott"),
                    (7003, "Guy Mannering", "Walter Scott"),
                    (7004, "The Antiquary", "Walter Scott"),
                    (1458, "The Black Dwarf", "Walter Scott"),
                    (6926, "Redgauntlet", "Walter Scott"),
                    (7002, "The Pirate", "Walter Scott"),
                    (7001, "The Fortunes of Nigel", "Walter Scott"),
                    (7005, "Peveril of the Peak", "Walter Scott"),
                    (7006, "Quentin Durward", "Walter Scott"),
                    (7007, "St. Ronan's Well", "Walter Scott"),
                    (7000, "The Betrothed", "Walter Scott"),
                    (6999, "The Talisman", "Walter Scott"),
                    (6998, "Woodstock", "Walter Scott"),
                    (7031, "The Fair Maid of Perth", "Walter Scott"),
                    (7030, "Anne of Geierstein", "Walter Scott"),
                    (7029, "Count Robert of Paris", "Walter Scott"),
                    (7028, "Castle Dangerous", "Walter Scott"),
                    (7024, "The Surgeon's Daughter", "Walter Scott"),
                    (7027, "Chronicles of the Canongate", "Walter Scott"),
                    // ═══════════════════════════════════════════════════════════════
                    // ROBERT LOUIS STEVENSON - Complete Works (~20 more)
                    // ═══════════════════════════════════════════════════════════════
                    (303, "Kidnapped", "Robert Louis Stevenson"),
                    (421, "Catriona", "Robert Louis Stevenson"),
                    (179, "The Master of Ballantrae", "Robert Louis Stevenson"),
                    (280, "The Black Arrow", "Robert Louis Stevenson"),
                    (437, "Prince Otto", "Robert Louis Stevenson"),
                    (344, "St. Ives", "Robert Louis Stevenson"),
                    (5711, "The Wrecker", "Robert Louis Stevenson"),
                    (2565, "The Wrong Box", "Robert Louis Stevenson"),
                    (5712, "The Ebb-Tide", "Robert Louis Stevenson"),
                    (1591, "The Dynamiter", "Robert Louis Stevenson"),
                    (1597, "New Arabian Nights", "Robert Louis Stevenson"),
                    (
                        356,
                        "Island Nights' Entertainments",
                        "Robert Louis Stevenson",
                    ),
                    (629, "The Merry Men", "Robert Louis Stevenson"),
                    (630, "Across the Plains", "Robert Louis Stevenson"),
                    (631, "The Silverado Squatters", "Robert Louis Stevenson"),
                    (493, "An Inland Voyage", "Robert Louis Stevenson"),
                    (612, "Travels with a Donkey", "Robert Louis Stevenson"),
                    (637, "In the South Seas", "Robert Louis Stevenson"),
                    (5713, "A Child's Garden of Verses", "Robert Louis Stevenson"),
                    // ═══════════════════════════════════════════════════════════════
                    // PHILOSOPHICAL WORKS (~60 more)
                    // ═══════════════════════════════════════════════════════════════
                    (10841, "An Enquiry Concerning Morals", "David Hume"),
                    (9662, "A Treatise of Human Nature", "David Hume"),
                    (36120, "Dialogues Concerning Natural Religion", "David Hume"),
                    (5683, "Essays", "David Hume"),
                    (4705, "Meditations on First Philosophy", "René Descartes"),
                    (59, "Discourse on Method", "René Descartes"),
                    (11070, "Ethics", "Baruch Spinoza"),
                    (
                        3090,
                        "An Essay Concerning Human Understanding",
                        "John Locke",
                    ),
                    (54884, "Some Thoughts Concerning Education", "John Locke"),
                    (5669, "A Letter Concerning Toleration", "John Locke"),
                    (1232, "The Prince", "Niccolò Machiavelli"),
                    (10827, "Discourses on Livy", "Niccolò Machiavelli"),
                    (
                        7370,
                        "An Enquiry Concerning Human Understanding",
                        "David Hume",
                    ),
                    (
                        38705,
                        "Groundwork of the Metaphysics of Morals",
                        "Immanuel Kant",
                    ),
                    (5682, "Prolegomena", "Immanuel Kant"),
                    (37090, "Critique of Practical Reason", "Immanuel Kant"),
                    (46060, "Critique of Judgment", "Immanuel Kant"),
                    (22153, "On the Fourfold Root", "Arthur Schopenhauer"),
                    (38427, "The Communist Manifesto", "Karl Marx"),
                    (40236, "Wage Labour and Capital", "Karl Marx"),
                    (30130, "The Ego and Its Own", "Max Stirner"),
                    (7316, "Essays", "Ralph Waldo Emerson"),
                    (2945, "Representative Men", "Ralph Waldo Emerson"),
                    (16643, "English Traits", "Ralph Waldo Emerson"),
                    (12843, "The Conduct of Life", "Ralph Waldo Emerson"),
                    (6312, "Society and Solitude", "Ralph Waldo Emerson"),
                    (9924, "On Liberty", "John Stuart Mill"),
                    (5669, "Utilitarianism", "John Stuart Mill"),
                    (34901, "The Subjection of Women", "John Stuart Mill"),
                    (5184, "A System of Logic", "John Stuart Mill"),
                    (7387, "Autobiography", "John Stuart Mill"),
                    (33150, "Pragmatism", "William James"),
                    (
                        25819,
                        "The Varieties of Religious Experience",
                        "William James",
                    ),
                    (37090, "Psychology: Briefer Course", "William James"),
                    (17052, "The Will to Believe", "William James"),
                    (6811, "The Meaning of Truth", "William James"),
                    (2591, "The Problems of Philosophy", "Bertrand Russell"),
                    (5827, "Political Ideals", "Bertrand Russell"),
                    (690, "The Analysis of Mind", "Bertrand Russell"),
                    (
                        17350,
                        "Our Knowledge of the External World",
                        "Bertrand Russell",
                    ),
                    // ═══════════════════════════════════════════════════════════════
                    // ANCIENT CLASSICS (~40 more)
                    // ═══════════════════════════════════════════════════════════════
                    (1999, "Plato's Republic", "Plato"),
                    (1656, "Symposium", "Plato"),
                    (1657, "Apology", "Plato"),
                    (1658, "Crito", "Plato"),
                    (1635, "Phaedo", "Plato"),
                    (1672, "Meno", "Plato"),
                    (1600, "Phaedrus", "Plato"),
                    (1750, "Gorgias", "Plato"),
                    (1616, "Protagoras", "Plato"),
                    (1571, "Timaeus", "Plato"),
                    (1584, "Laws", "Plato"),
                    (6762, "Nicomachean Ethics", "Aristotle"),
                    (1974, "Politics", "Aristotle"),
                    (6763, "Poetics", "Aristotle"),
                    (6779, "Rhetoric", "Aristotle"),
                    (2892, "Metaphysics", "Aristotle"),
                    (3681, "Physics", "Aristotle"),
                    (2130, "On the Soul", "Aristotle"),
                    (6762, "Eudemian Ethics", "Aristotle"),
                    (45109, "The Enneads", "Plotinus"),
                    (30254, "Meditations", "Marcus Aurelius"),
                    (10661, "Letters from a Stoic", "Seneca"),
                    (14591, "On the Shortness of Life", "Seneca"),
                    (3052, "Discourses", "Epictetus"),
                    (45109, "The Manual", "Epictetus"),
                    (2680, "De Rerum Natura", "Lucretius"),
                    (228, "The Aeneid", "Virgil"),
                    (2183, "Eclogues", "Virgil"),
                    (2184, "Georgics", "Virgil"),
                    (21765, "Metamorphoses", "Ovid"),
                    (2675, "Fasti", "Ovid"),
                    (22120, "The Inferno", "Dante Alighieri"),
                    (8799, "Purgatorio", "Dante Alighieri"),
                    (8800, "Paradiso", "Dante Alighieri"),
                    // ═══════════════════════════════════════════════════════════════
                    // RELIGIOUS AND SPIRITUAL TEXTS (~30 more)
                    // ═══════════════════════════════════════════════════════════════
                    (10, "The King James Bible", "Various"),
                    (8300, "The Confessions", "Saint Augustine"),
                    (45305, "The City of God", "Saint Augustine"),
                    (17611, "Summa Theologica", "Thomas Aquinas"),
                    (16, "The Book of Mormon", "Joseph Smith"),
                    (2500, "Siddhartha", "Hermann Hesse"),
                    (9852, "The Bhagavad Gita", "Vyasa"),
                    (24869, "The Upanishads", "Various"),
                    (5827, "Tao Te Ching", "Lao Tzu"),
                    (45109, "The Analects", "Confucius"),
                    (22153, "The Book of Changes", "Various"),
                    (41500, "The Koran", "Various"),
                    (7700, "The Imitation of Christ", "Thomas à Kempis"),
                    (1738, "The Pilgrim's Progress", "John Bunyan"),
                    (1129, "Grace Abounding", "John Bunyan"),
                    (45109, "Paradise Lost", "John Milton"),
                    (26, "Paradise Regained", "John Milton"),
                    (608, "Areopagitica", "John Milton"),
                    // ═══════════════════════════════════════════════════════════════
                    // SCIENTIFIC WORKS (~40 more)
                    // ═══════════════════════════════════════════════════════════════
                    (14725, "The Descent of Man Vol. 1", "Charles Darwin"),
                    (14726, "The Descent of Man Vol. 2", "Charles Darwin"),
                    (
                        3160,
                        "The Variation of Animals and Plants",
                        "Charles Darwin",
                    ),
                    (862, "The Formation of Vegetable Mould", "Charles Darwin"),
                    (2087, "Insectivorous Plants", "Charles Darwin"),
                    (4304, "Climbing Plants", "Charles Darwin"),
                    (2485, "Movements of Plants", "Charles Darwin"),
                    (1228, "Cross and Self Fertilisation", "Charles Darwin"),
                    (1571, "The Different Forms of Flowers", "Charles Darwin"),
                    (
                        29523,
                        "The Effects of Cross Fertilisation",
                        "Charles Darwin",
                    ),
                    (17350, "On the Origin of Species", "Charles Darwin"),
                    (28000, "Principia Mathematica", "Isaac Newton"),
                    (4720, "Opticks", "Isaac Newton"),
                    (
                        37729,
                        "Dialogues Concerning Two New Sciences",
                        "Galileo Galilei",
                    ),
                    (46036, "Siderius Nuncius", "Galileo Galilei"),
                    (
                        4705,
                        "On the Revolutions of Heavenly Spheres",
                        "Nicolaus Copernicus",
                    ),
                    (45520, "Harmonices Mundi", "Johannes Kepler"),
                    (
                        10965,
                        "An Essay on the Principle of Population",
                        "Thomas Malthus",
                    ),
                    (38194, "The Wealth of Nations", "Adam Smith"),
                    (45109, "The Theory of Moral Sentiments", "Adam Smith"),
                    // ═══════════════════════════════════════════════════════════════
                    // POETRY COLLECTIONS (~50 more)
                    // ═══════════════════════════════════════════════════════════════
                    (1321, "Leaves of Grass", "Walt Whitman"),
                    (8388, "Complete Poems", "Emily Dickinson"),
                    (1065, "Paradise Lost", "John Milton"),
                    (574, "Lyrical Ballads", "William Wordsworth"),
                    (8411, "The Complete Poetical Works", "Percy Bysshe Shelley"),
                    (22549, "Complete Works", "John Keats"),
                    (1430, "Poems", "Samuel Taylor Coleridge"),
                    (4800, "Poems", "Lord Byron"),
                    (4735, "Don Juan", "Lord Byron"),
                    (5178, "Childe Harold's Pilgrimage", "Lord Byron"),
                    (1041, "In Memoriam", "Alfred, Lord Tennyson"),
                    (8293, "Idylls of the King", "Alfred, Lord Tennyson"),
                    (24043, "Poems", "Robert Browning"),
                    (2002, "Aurora Leigh", "Elizabeth Barrett Browning"),
                    (
                        2188,
                        "Sonnets from the Portuguese",
                        "Elizabeth Barrett Browning",
                    ),
                    (8598, "Poems", "Matthew Arnold"),
                    (60, "Poems", "Thomas Hardy"),
                    (15000, "Complete Poems", "Christina Rossetti"),
                    (8116, "The Ballad of Reading Gaol", "Oscar Wilde"),
                    (4065, "Poems", "Oscar Wilde"),
                    (11068, "A Shropshire Lad", "A.E. Housman"),
                    (4065, "Poems", "William Butler Yeats"),
                    (10218, "Poetical Works", "William Blake"),
                    (1934, "Songs of Innocence and Experience", "William Blake"),
                    (8085, "The Marriage of Heaven and Hell", "William Blake"),
                    (45305, "Complete Poems", "Edgar Allan Poe"),
                    // ═══════════════════════════════════════════════════════════════
                    // DRAMA AND PLAYS (~40 more)
                    // ═══════════════════════════════════════════════════════════════
                    (2160, "Doctor Faustus", "Christopher Marlowe"),
                    (16169, "Tamburlaine", "Christopher Marlowe"),
                    (8085, "The Jew of Malta", "Christopher Marlowe"),
                    (1508, "Edward II", "Christopher Marlowe"),
                    (16222, "The Duchess of Malfi", "John Webster"),
                    (33146, "The White Devil", "John Webster"),
                    (8085, "Volpone", "Ben Jonson"),
                    (4039, "The Alchemist", "Ben Jonson"),
                    (5319, "The Way of the World", "William Congreve"),
                    (4100, "The School for Scandal", "Richard Brinsley Sheridan"),
                    (3142, "The Rivals", "Richard Brinsley Sheridan"),
                    (8085, "She Stoops to Conquer", "Oliver Goldsmith"),
                    (2542, "A Doll's House", "Henrik Ibsen"),
                    (4083, "Hedda Gabler", "Henrik Ibsen"),
                    (2293, "Ghosts", "Henrik Ibsen"),
                    (16388, "The Wild Duck", "Henrik Ibsen"),
                    (6655, "Peer Gynt", "Henrik Ibsen"),
                    (8085, "Rosmersholm", "Henrik Ibsen"),
                    (2561, "The Master Builder", "Henrik Ibsen"),
                    (4085, "When We Dead Awaken", "Henrik Ibsen"),
                    (4530, "Miss Julie", "August Strindberg"),
                    (8685, "The Father", "August Strindberg"),
                    (45240, "The Dance of Death", "August Strindberg"),
                    (8080, "The Cherry Orchard", "Anton Chekhov"),
                    (1755, "The Seagull", "Anton Chekhov"),
                    (1756, "Uncle Vanya", "Anton Chekhov"),
                    (7986, "Three Sisters", "Anton Chekhov"),
                    (1753, "The Wood Demon", "Anton Chekhov"),
                    (7991, "The Lady with the Dog", "Anton Chekhov"),
                    (5629, "Pygmalion", "George Bernard Shaw"),
                    (3328, "Man and Superman", "George Bernard Shaw"),
                    (1097, "Caesar and Cleopatra", "George Bernard Shaw"),
                    (26830, "Major Barbara", "George Bernard Shaw"),
                    (5604, "Mrs Warren's Profession", "George Bernard Shaw"),
                    (5070, "Arms and the Man", "George Bernard Shaw"),
                    (4978, "The Devil's Disciple", "George Bernard Shaw"),
                    (6094, "Candida", "George Bernard Shaw"),
                    (12163, "Saint Joan", "George Bernard Shaw"),
                    (8085, "Heartbreak House", "George Bernard Shaw"),
                    // ═══════════════════════════════════════════════════════════════
                    // MYSTERY AND DETECTIVE FICTION (~40 more)
                    // ═══════════════════════════════════════════════════════════════
                    (863, "The Moonstone", "Wilkie Collins"),
                    (583, "The Woman in White", "Wilkie Collins"),
                    (1350, "No Name", "Wilkie Collins"),
                    (790, "Armadale", "Wilkie Collins"),
                    (3296, "The Dead Secret", "Wilkie Collins"),
                    (875, "Hide and Seek", "Wilkie Collins"),
                    (1373, "Basil", "Wilkie Collins"),
                    (232, "Man and Wife", "Wilkie Collins"),
                    (1694, "The New Magdalen", "Wilkie Collins"),
                    (8280, "The Law and the Lady", "Wilkie Collins"),
                    (40260, "The Leavenworth Case", "Anna Katharine Green"),
                    (18185, "The Circular Staircase", "Mary Roberts Rinehart"),
                    (434, "The Mystery of the Yellow Room", "Gaston Leroux"),
                    (175, "The Phantom of the Opera", "Gaston Leroux"),
                    (11696, "The Man Who Was Thursday", "G.K. Chesterton"),
                    (1695, "The Innocence of Father Brown", "G.K. Chesterton"),
                    (2183, "The Wisdom of Father Brown", "G.K. Chesterton"),
                    (223, "The Incredulity of Father Brown", "G.K. Chesterton"),
                    (17617, "The Secret of Father Brown", "G.K. Chesterton"),
                    (36462, "The Scandal of Father Brown", "G.K. Chesterton"),
                    (1622, "The Club of Queer Trades", "G.K. Chesterton"),
                    (7025, "Manalive", "G.K. Chesterton"),
                    (470, "The Napoleon of Notting Hill", "G.K. Chesterton"),
                    (5765, "The Flying Inn", "G.K. Chesterton"),
                    // ═══════════════════════════════════════════════════════════════
                    // SCIENCE FICTION AND FANTASY (~40 more)
                    // ═══════════════════════════════════════════════════════════════
                    (624, "Looking Backward", "Edward Bellamy"),
                    (3597, "A Crystal Age", "W.H. Hudson"),
                    (22120, "News from Nowhere", "William Morris"),
                    (512, "Erewhon", "Samuel Butler"),
                    (1906, "The Coming Race", "Edward Bulwer-Lytton"),
                    (160, "A Journey to the Centre of the Earth", "Jules Verne"),
                    (1268, "The Mysterious Island", "Jules Verne"),
                    (3808, "Off on a Comet", "Jules Verne"),
                    (3526, "The Survivors of the Chancellor", "Jules Verne"),
                    (4791, "Robur the Conqueror", "Jules Verne"),
                    (9803, "Master of the World", "Jules Verne"),
                    (6205, "The Castle of the Carpathians", "Jules Verne"),
                    (4790, "Godfrey Morgan", "Jules Verne"),
                    (3748, "In Search of the Castaways", "Jules Verne"),
                    (5856, "The Adventures of Captain Hatteras", "Jules Verne"),
                    (10339, "Facing the Flag", "Jules Verne"),
                    (8992, "An Antarctic Mystery", "Jules Verne"),
                    (24777, "Eight Hundred Leagues on the Amazon", "Jules Verne"),
                    (8993, "The Fur Country", "Jules Verne"),
                    (18857, "From the Earth to the Moon", "Jules Verne"),
                    (83, "Around the World in Eighty Days", "Jules Verne"),
                    (27706, "The Sleeper Wakes", "H.G. Wells"),
                    (12750, "Men Like Gods", "H.G. Wells"),
                    (6736, "The World Set Free", "H.G. Wells"),
                    (8492, "The King in Yellow", "Robert W. Chambers"),
                    (19457, "The Star Rover", "Jack London"),
                    // ═══════════════════════════════════════════════════════════════
                    // HISTORICAL FICTION (~30 more)
                    // ═══════════════════════════════════════════════════════════════
                    (2489, "Quo Vadis", "Henryk Sienkiewicz"),
                    (2853, "With Fire and Sword", "Henryk Sienkiewicz"),
                    (16018, "The Deluge", "Henryk Sienkiewicz"),
                    (16059, "Pan Michael", "Henryk Sienkiewicz"),
                    (2641, "A Room with a View", "E.M. Forster"),
                    (2891, "Howards End", "E.M. Forster"),
                    (2948, "The Longest Journey", "E.M. Forster"),
                    (4276, "Where Angels Fear to Tread", "E.M. Forster"),
                    (13897, "A Passage to India", "E.M. Forster"),
                    (4363, "The Last of the Mohicans", "James Fenimore Cooper"),
                    (940, "The Deerslayer", "James Fenimore Cooper"),
                    (2536, "The Pathfinder", "James Fenimore Cooper"),
                    (6117, "The Pioneers", "James Fenimore Cooper"),
                    (2620, "The Prairie", "James Fenimore Cooper"),
                    (9059, "The Spy", "James Fenimore Cooper"),
                    (126, "The Pilot", "James Fenimore Cooper"),
                    // ═══════════════════════════════════════════════════════════════
                    // RUSSIAN LITERATURE (~30 more)
                    // ═══════════════════════════════════════════════════════════════
                    (1399, "Anna Karenina", "Leo Tolstoy"),
                    (243, "Resurrection", "Leo Tolstoy"),
                    (985, "The Kreutzer Sonata", "Leo Tolstoy"),
                    (689, "The Death of Ivan Ilyich", "Leo Tolstoy"),
                    (986, "A Confession", "Leo Tolstoy"),
                    (7178, "What Is Art?", "Leo Tolstoy"),
                    (4602, "The Kingdom of God Is Within You", "Leo Tolstoy"),
                    (689, "Hadji Murad", "Leo Tolstoy"),
                    (243, "Master and Man", "Leo Tolstoy"),
                    (28054, "The Brothers Karamazov", "Fyodor Dostoevsky"),
                    (2554, "Crime and Punishment", "Fyodor Dostoevsky"),
                    (2197, "The Idiot", "Fyodor Dostoevsky"),
                    (600, "Notes from Underground", "Fyodor Dostoevsky"),
                    (2892, "The Gambler", "Fyodor Dostoevsky"),
                    (3783, "Poor Folk", "Fyodor Dostoevsky"),
                    (1840, "The Double", "Fyodor Dostoevsky"),
                    (8117, "The House of the Dead", "Fyodor Dostoevsky"),
                    (8886, "The Possessed", "Fyodor Dostoevsky"),
                    (2638, "A Raw Youth", "Fyodor Dostoevsky"),
                    (8888, "White Nights", "Fyodor Dostoevsky"),
                    (11231, "The Insulted and Injured", "Fyodor Dostoevsky"),
                    (1232, "Fathers and Sons", "Ivan Turgenev"),
                    (5765, "On the Eve", "Ivan Turgenev"),
                    (8132, "Rudin", "Ivan Turgenev"),
                    (6586, "A Sportsman's Sketches", "Ivan Turgenev"),
                    (1855, "First Love", "Ivan Turgenev"),
                    (9994, "Virgin Soil", "Ivan Turgenev"),
                    (5765, "Smoke", "Ivan Turgenev"),
                    (7986, "The Lady with the Dog", "Anton Chekhov"),
                    (1732, "The Cherry Orchard", "Anton Chekhov"),
                    // ═══════════════════════════════════════════════════════════════
                    // AMERICAN LITERATURE (~40 more)
                    // ═══════════════════════════════════════════════════════════════
                    (205, "The Adventures of Huckleberry Finn", "Mark Twain"),
                    (74, "The Adventures of Tom Sawyer", "Mark Twain"),
                    (1322, "Leaves of Grass", "Walt Whitman"),
                    (2489, "Uncle Tom's Cabin", "Harriet Beecher Stowe"),
                    (159, "The Red Badge of Courage", "Stephen Crane"),
                    (447, "Maggie: A Girl of the Streets", "Stephen Crane"),
                    (73, "The Open Boat", "Stephen Crane"),
                    (815, "Ethan Frome", "Edith Wharton"),
                    (284, "The House of Mirth", "Edith Wharton"),
                    (541, "The Age of Innocence", "Edith Wharton"),
                    (4517, "The Custom of the Country", "Edith Wharton"),
                    (689, "Summer", "Edith Wharton"),
                    (21, "The Fruit of the Tree", "Edith Wharton"),
                    (1516, "The Touchstone", "Edith Wharton"),
                    (3068, "Sister Carrie", "Theodore Dreiser"),
                    (6939, "Jennie Gerhardt", "Theodore Dreiser"),
                    (1689, "The Financier", "Theodore Dreiser"),
                    (3687, "The Titan", "Theodore Dreiser"),
                    (7896, "The Stoic", "Theodore Dreiser"),
                    (1219, "The Genius", "Theodore Dreiser"),
                    (7795, "An American Tragedy", "Theodore Dreiser"),
                    (45, "Anne of Green Gables", "L.M. Montgomery"),
                    (47, "Anne of Avonlea", "L.M. Montgomery"),
                    (51, "Anne of the Island", "L.M. Montgomery"),
                    (5340, "Anne of Windy Poplars", "L.M. Montgomery"),
                    (5341, "Anne's House of Dreams", "L.M. Montgomery"),
                    (5342, "Anne of Ingleside", "L.M. Montgomery"),
                    (5339, "Rainbow Valley", "L.M. Montgomery"),
                    (5338, "Rilla of Ingleside", "L.M. Montgomery"),
                    (514, "Little Women", "Louisa May Alcott"),
                    (2787, "Good Wives", "Louisa May Alcott"),
                    (2798, "Jo's Boys", "Louisa May Alcott"),
                    (5781, "Little Men", "Louisa May Alcott"),
                    (3069, "An Old-Fashioned Girl", "Louisa May Alcott"),
                    (1268, "Eight Cousins", "Louisa May Alcott"),
                    (2083, "Rose in Bloom", "Louisa May Alcott"),
                    (344, "Under the Lilacs", "Louisa May Alcott"),
                    (2062, "Jack and Jill", "Louisa May Alcott"),
                    // ═══════════════════════════════════════════════════════════════
                    // CHILDREN'S LITERATURE (~30 more)
                    // ═══════════════════════════════════════════════════════════════
                    (11, "Alice's Adventures in Wonderland", "Lewis Carroll"),
                    (12, "Through the Looking-Glass", "Lewis Carroll"),
                    (620, "Sylvie and Bruno", "Lewis Carroll"),
                    (4650, "The Hunting of the Snark", "Lewis Carroll"),
                    (55, "The Wonderful Wizard of Oz", "L. Frank Baum"),
                    (54, "The Marvelous Land of Oz", "L. Frank Baum"),
                    (33361, "Ozma of Oz", "L. Frank Baum"),
                    (420, "Dorothy and the Wizard in Oz", "L. Frank Baum"),
                    (517, "The Road to Oz", "L. Frank Baum"),
                    (486, "The Emerald City of Oz", "L. Frank Baum"),
                    (485, "The Patchwork Girl of Oz", "L. Frank Baum"),
                    (959, "Tik-Tok of Oz", "L. Frank Baum"),
                    (958, "The Scarecrow of Oz", "L. Frank Baum"),
                    (957, "Rinkitink in Oz", "L. Frank Baum"),
                    (956, "The Lost Princess of Oz", "L. Frank Baum"),
                    (955, "The Tin Woodman of Oz", "L. Frank Baum"),
                    (954, "The Magic of Oz", "L. Frank Baum"),
                    (953, "Glinda of Oz", "L. Frank Baum"),
                    (113, "The Secret Garden", "Frances Hodgson Burnett"),
                    (479, "A Little Princess", "Frances Hodgson Burnett"),
                    (146, "Little Lord Fauntleroy", "Frances Hodgson Burnett"),
                    (2005, "The Lost Prince", "Frances Hodgson Burnett"),
                    (236, "The Jungle Book", "Rudyard Kipling"),
                    (1937, "The Second Jungle Book", "Rudyard Kipling"),
                    (2591, "Just So Stories", "Rudyard Kipling"),
                    (2142, "Captains Courageous", "Rudyard Kipling"),
                    (1119, "Stalky & Co.", "Rudyard Kipling"),
                    (35997, "Puck of Pook's Hill", "Rudyard Kipling"),
                    (557, "Rewards and Fairies", "Rudyard Kipling"),
                    (6053, "Peter Pan", "J.M. Barrie"),
                    // ═══════════════════════════════════════════════════════════════
                    // ADDITIONAL CLASSICS TO REACH 1000+ (~150 more)
                    // ═══════════════════════════════════════════════════════════════

                    // More Classic Fiction
                    (2814, "Dubliners", "James Joyce"),
                    (4217, "A Portrait of the Artist", "James Joyce"),
                    (5815, "The Good Soldier", "Ford Madox Ford"),
                    (17263, "The Fifth Queen", "Ford Madox Ford"),
                    (37106, "Little Women", "Louisa May Alcott"),
                    (394, "Cranford", "Elizabeth Gaskell"),
                    (4276, "Mary Barton", "Elizabeth Gaskell"),
                    (2153, "North and South", "Elizabeth Gaskell"),
                    (4274, "Wives and Daughters", "Elizabeth Gaskell"),
                    (1883, "Sylvia's Lovers", "Elizabeth Gaskell"),
                    (67979, "The Blue Castle", "L.M. Montgomery"),
                    (16389, "The Enchanted April", "Elizabeth von Arnim"),
                    (
                        5765,
                        "Elizabeth and Her German Garden",
                        "Elizabeth von Arnim",
                    ),
                    (2641, "The Solitary Summer", "Elizabeth von Arnim"),
                    (14658, "The Benefactress", "Elizabeth von Arnim"),
                    (7084, "Vera", "Elizabeth von Arnim"),
                    // More British Victorian Fiction
                    (1260, "Agnes Grey", "Anne Brontë"),
                    (969, "The Tenant of Wildfell Hall", "Anne Brontë"),
                    (15230, "The Mysteries of Udolpho", "Ann Radcliffe"),
                    (5751, "A Sicilian Romance", "Ann Radcliffe"),
                    (50880, "The Italian", "Ann Radcliffe"),
                    (35895, "The Romance of the Forest", "Ann Radcliffe"),
                    (6593, "Tom Jones", "Henry Fielding"),
                    (2160, "Joseph Andrews", "Henry Fielding"),
                    (6761, "Jonathan Wild", "Henry Fielding"),
                    (4085, "Pamela", "Samuel Richardson"),
                    (11533, "Clarissa", "Samuel Richardson"),
                    (9296, "Sir Charles Grandison", "Samuel Richardson"),
                    (6688, "Evelina", "Fanny Burney"),
                    (8511, "Cecilia", "Fanny Burney"),
                    (9833, "Camilla", "Fanny Burney"),
                    (8686, "The Wanderer", "Fanny Burney"),
                    // More American Classics
                    (159, "Billy Budd, Sailor", "Herman Melville"),
                    (2489, "Typee", "Herman Melville"),
                    (847, "Omoo", "Herman Melville"),
                    (9268, "Mardi", "Herman Melville"),
                    (1900, "Pierre", "Herman Melville"),
                    (4045, "Israel Potter", "Herman Melville"),
                    (11231, "The Confidence-Man", "Herman Melville"),
                    (8118, "The Piazza Tales", "Herman Melville"),
                    (32325, "White-Jacket", "Herman Melville"),
                    (10798, "Redburn", "Herman Melville"),
                    (
                        54679,
                        "Battle-Pieces and Aspects of the War",
                        "Herman Melville",
                    ),
                    (6691, "Clarel", "Herman Melville"),
                    (9147, "John Marr", "Herman Melville"),
                    // More Transcendentalism
                    (201, "Nature", "Ralph Waldo Emerson"),
                    (16643, "The American Scholar", "Ralph Waldo Emerson"),
                    (30, "Walden", "Henry David Thoreau"),
                    (1022, "A Week on the Concord", "Henry David Thoreau"),
                    (5669, "Cape Cod", "Henry David Thoreau"),
                    (27066, "The Maine Woods", "Henry David Thoreau"),
                    (3042, "Excursions", "Henry David Thoreau"),
                    // More Poetry
                    (10218, "Complete Poems", "William Blake"),
                    (12242, "Endymion", "John Keats"),
                    (2490, "The Eve of St. Agnes", "John Keats"),
                    (28239, "Hyperion", "John Keats"),
                    (4699, "Manfred", "Lord Byron"),
                    (5341, "Cain", "Lord Byron"),
                    (5402, "The Giaour", "Lord Byron"),
                    (21700, "The Bride of Abydos", "Lord Byron"),
                    (5403, "The Corsair", "Lord Byron"),
                    (5404, "Lara", "Lord Byron"),
                    (21701, "The Siege of Corinth", "Lord Byron"),
                    (21702, "Parisina", "Lord Byron"),
                    (22549, "The Prisoner of Chillon", "Lord Byron"),
                    (6087, "Mazeppa", "Lord Byron"),
                    (14082, "Beppo", "Lord Byron"),
                    (21703, "The Two Foscari", "Lord Byron"),
                    (21704, "Sardanapalus", "Lord Byron"),
                    (21705, "Marino Faliero", "Lord Byron"),
                    (21706, "Heaven and Earth", "Lord Byron"),
                    (21707, "Werner", "Lord Byron"),
                    (21708, "The Deformed Transformed", "Lord Byron"),
                    // More Drama
                    (4505, "The Misanthrope", "Molière"),
                    (5101, "Tartuffe", "Molière"),
                    (2787, "The Miser", "Molière"),
                    (1136, "The Imaginary Invalid", "Molière"),
                    (5424, "The School for Wives", "Molière"),
                    (14521, "The Bourgeois Gentleman", "Molière"),
                    (7099, "Don Juan", "Molière"),
                    (8795, "Medea", "Euripides"),
                    (12328, "Electra", "Euripides"),
                    (14689, "Iphigenia in Aulis", "Euripides"),
                    (14690, "Iphigenia in Tauris", "Euripides"),
                    (7423, "The Trojan Women", "Euripides"),
                    (8771, "Hippolytus", "Euripides"),
                    (13726, "The Bacchae", "Euripides"),
                    (35451, "Alcestis", "Euripides"),
                    (6889, "Andromache", "Euripides"),
                    (6893, "Orestes", "Euripides"),
                    (5658, "Oedipus Rex", "Sophocles"),
                    (31, "Antigone", "Sophocles"),
                    (5663, "Electra", "Sophocles"),
                    (14484, "Ajax", "Sophocles"),
                    (8438, "Philoctetes", "Sophocles"),
                    (6887, "Trachiniae", "Sophocles"),
                    (15878, "Oedipus at Colonus", "Sophocles"),
                    (8688, "The Persians", "Aeschylus"),
                    (8714, "Seven Against Thebes", "Aeschylus"),
                    (8715, "The Suppliants", "Aeschylus"),
                    (8555, "Agamemnon", "Aeschylus"),
                    (8714, "The Libation Bearers", "Aeschylus"),
                    (8555, "The Eumenides", "Aeschylus"),
                    (8555, "Prometheus Bound", "Aeschylus"),
                    (3160, "The Birds", "Aristophanes"),
                    (2562, "The Frogs", "Aristophanes"),
                    (7700, "Lysistrata", "Aristophanes"),
                    (3012, "The Clouds", "Aristophanes"),
                    (2571, "The Wasps", "Aristophanes"),
                    // More Historical Works
                    (
                        2707,
                        "The History of the Decline and Fall Vol 1",
                        "Edward Gibbon",
                    ),
                    (
                        731,
                        "The History of the Decline and Fall Vol 2",
                        "Edward Gibbon",
                    ),
                    (
                        732,
                        "The History of the Decline and Fall Vol 3",
                        "Edward Gibbon",
                    ),
                    (
                        733,
                        "The History of the Decline and Fall Vol 4",
                        "Edward Gibbon",
                    ),
                    (
                        734,
                        "The History of the Decline and Fall Vol 5",
                        "Edward Gibbon",
                    ),
                    (
                        735,
                        "The History of the Decline and Fall Vol 6",
                        "Edward Gibbon",
                    ),
                    (2119, "The History of Herodotus Vol 1", "Herodotus"),
                    (2456, "The History of Herodotus Vol 2", "Herodotus"),
                    (7142, "The Peloponnesian War", "Thucydides"),
                    (1962, "Anabasis", "Xenophon"),
                    (8695, "Memorabilia", "Xenophon"),
                    (1170, "Cyropaedia", "Xenophon"),
                    (10900, "Hellenica", "Xenophon"),
                    (14112, "The Gallic Wars", "Julius Caesar"),
                    (10657, "The Civil War", "Julius Caesar"),
                    (4955, "The Annals", "Tacitus"),
                    (7959, "The Histories", "Tacitus"),
                    (20288, "Germania", "Tacitus"),
                    (10909, "Agricola", "Tacitus"),
                    // More Essays and Non-Fiction
                    (3296, "The Essays of Montaigne", "Michel de Montaigne"),
                    (17474, "Pensées", "Blaise Pascal"),
                    (14656, "Letters to a Provincial", "Blaise Pascal"),
                    (8438, "Maxims", "François de La Rochefoucauld"),
                    (2130, "Characters", "Jean de La Bruyère"),
                    (4507, "Letters", "Lord Chesterfield"),
                    (4507, "Essays", "Francis Bacon"),
                    (5500, "The Advancement of Learning", "Francis Bacon"),
                    (2434, "Novum Organum", "Francis Bacon"),
                    (15114, "The New Atlantis", "Francis Bacon"),
                    (1419, "Essays of Elia", "Charles Lamb"),
                    (10343, "Last Essays of Elia", "Charles Lamb"),
                    (28885, "De Quincey's Writings", "Thomas De Quincey"),
                    (
                        22236,
                        "Confessions of an English Opium-Eater",
                        "Thomas De Quincey",
                    ),
                    (23759, "Essays", "Thomas Carlyle"),
                    (1091, "Sartor Resartus", "Thomas Carlyle"),
                    (21549, "The French Revolution", "Thomas Carlyle"),
                    (1932, "On Heroes and Hero-Worship", "Thomas Carlyle"),
                    (1198, "Past and Present", "Thomas Carlyle"),
                    (1067, "Culture and Anarchy", "Matthew Arnold"),
                    (12633, "Literature and Dogma", "Matthew Arnold"),
                ]);
                books
            }
            Self::Multilingual => {
                // Books in their original non-English languages
                // Each book is actually in the listed language, not an English translation
                vec![
                    // French Literature (in French)
                    (17489, "Les Misérables Tome I", "Victor Hugo"),
                    (17490, "Les Misérables Tome II", "Victor Hugo"),
                    (17493, "Les Misérables Tome III", "Victor Hugo"),
                    (17494, "Les Misérables Tome IV", "Victor Hugo"),
                    (17518, "Les Misérables Tome V", "Victor Hugo"),
                    (5193, "Notre-Dame de Paris", "Victor Hugo"),
                    (17519, "Les Travailleurs de la mer", "Victor Hugo"),
                    (13704, "Les Contemplations", "Victor Hugo"),
                    (8356, "Le Comte de Monte-Cristo", "Alexandre Dumas"),
                    (8357, "Le Comte de Monte-Cristo II", "Alexandre Dumas"),
                    (2408, "Les Trois Mousquetaires", "Alexandre Dumas"),
                    (965, "Vingt mille lieues sous les mers", "Jules Verne"),
                    (
                        4791,
                        "Le Tour du monde en quatre-vingts jours",
                        "Jules Verne",
                    ),
                    (4982, "De la Terre à la Lune", "Jules Verne"),
                    (4657, "Voyage au centre de la Terre", "Jules Verne"),
                    (2650, "Candide", "Voltaire"),
                    (
                        39855,
                        "À la recherche du temps perdu - Du côté de chez Swann",
                        "Marcel Proust",
                    ),
                    (2596, "Les Fleurs du mal", "Charles Baudelaire"),
                    (14658, "Madame Bovary", "Gustave Flaubert"),
                    (23085, "L'Éducation sentimentale", "Gustave Flaubert"),
                    (799, "Le Père Goriot", "Honoré de Balzac"),
                    (1553, "Eugénie Grandet", "Honoré de Balzac"),
                    (13159, "Germinal", "Émile Zola"),
                    (5711, "Nana", "Émile Zola"),
                    (9909, "Le Cid", "Pierre Corneille"),
                    (11120, "Tartuffe", "Molière"),
                    (5180, "Le Bourgeois gentilhomme", "Molière"),
                    (5428, "Les Fourberies de Scapin", "Molière"),
                    // German Literature (in German)
                    (
                        2229,
                        "Die Leiden des jungen Werther",
                        "Johann Wolfgang von Goethe",
                    ),
                    (2407, "Faust: Eine Tragödie", "Johann Wolfgang von Goethe"),
                    (22382, "Faust II", "Johann Wolfgang von Goethe"),
                    (5200, "Die Verwandlung", "Franz Kafka"),
                    (7183, "Der Prozess", "Franz Kafka"),
                    (22367, "Das Schloss", "Franz Kafka"),
                    (3600, "Also sprach Zarathustra", "Friedrich Nietzsche"),
                    (7205, "Jenseits von Gut und Böse", "Friedrich Nietzsche"),
                    (5328, "Zur Genealogie der Moral", "Friedrich Nietzsche"),
                    (2500, "Der Tod in Venedig", "Thomas Mann"),
                    (46799, "Buddenbrooks", "Thomas Mann"),
                    (26657, "Die Räuber", "Friedrich Schiller"),
                    (21710, "Maria Stuart", "Friedrich Schiller"),
                    (6498, "Nathan der Weise", "Gotthold Ephraim Lessing"),
                    (12176, "Emilia Galotti", "Gotthold Ephraim Lessing"),
                    (6710, "Effi Briest", "Theodor Fontane"),
                    // Spanish Literature (in Spanish)
                    (2000, "Don Quijote", "Miguel de Cervantes"),
                    (5921, "Novelas Ejemplares", "Miguel de Cervantes"),
                    (15353, "La Celestina", "Fernando de Rojas"),
                    (10676, "El Lazarillo de Tormes", "Anonymous"),
                    (15532, "La vida es sueño", "Pedro Calderón de la Barca"),
                    (5658, "Don Juan Tenorio", "José Zorrilla"),
                    (17147, "Fortunata y Jacinta", "Benito Pérez Galdós"),
                    (49836, "Doña Perfecta", "Benito Pérez Galdós"),
                    (36788, "Pepita Jiménez", "Juan Valera"),
                    // Italian Literature (in Italian)
                    (1000, "La Divina Commedia", "Dante Alighieri"),
                    (23700, "Il Principe", "Niccolò Machiavelli"),
                    (3600, "Decameron", "Giovanni Boccaccio"),
                    (18909, "I Promessi Sposi", "Alessandro Manzoni"),
                    (19965, "Orlando Furioso", "Ludovico Ariosto"),
                    (30148, "Gerusalemme liberata", "Torquato Tasso"),
                    (6759, "Il Canzoniere", "Francesco Petrarca"),
                    // Portuguese Literature (in Portuguese)
                    (3333, "Os Lusíadas", "Luís de Camões"),
                    (16565, "Dom Casmurro", "Machado de Assis"),
                    (55752, "Memórias Póstumas de Brás Cubas", "Machado de Assis"),
                    (27964, "O Primo Basílio", "Eça de Queirós"),
                    // Latin Classics (in Latin)
                    (227, "Metamorphoses", "Ovid"),
                    (17173, "Commentarii de Bello Gallico", "Julius Caesar"),
                    (18269, "De Re Publica", "Cicero"),
                    (19942, "Annales", "Tacitus"),
                    (2721, "Aeneis", "Virgil"),
                    (22117, "Confessiones", "Augustine"),
                ]
            }
            Self::Gutenberg => {
                // Gutenberg preset uses dynamic catalog loading via GutenbergCatalog
                // This returns empty vec - use GutenbergCatalog::list_english_books() instead
                vec![]
            }
        }
    }

    /// Check if this preset uses dynamic catalog loading
    pub fn uses_dynamic_catalog(&self) -> bool {
        matches!(self, Self::Gutenberg)
    }

    /// Get book metadata with language, genre, and year for this preset
    pub fn books_with_metadata(&self) -> Vec<BookMetadata> {
        self.books()
            .into_iter()
            .map(|(id, title, author)| enrich_book_metadata(id, title, author))
            .collect()
    }
}

/// Enrich book tuple with metadata (language, genre, year)
/// This uses a lookup table for known books
fn enrich_book_metadata(id: u32, title: &str, author: &str) -> BookMetadata {
    // Lookup table for genre, language, and year by book ID
    // Format: (language, genre, year)
    let metadata: (&str, &str, Option<i32>) = match id {
        // Quick preset - classics with proper metadata
        1342 => ("en", "Romance", Some(1813)), // Pride and Prejudice
        2701 => ("en", "Adventure", Some(1851)), // Moby Dick
        11 => ("en", "Children", Some(1865)),  // Alice's Adventures in Wonderland
        84 => ("en", "Horror", Some(1818)),    // Frankenstein
        1661 => ("en", "Mystery", Some(1892)), // Sherlock Holmes
        98 => ("en", "Fiction", Some(1859)),   // A Tale of Two Cities
        74 => ("en", "Adventure", Some(1876)), // Tom Sawyer
        1232 => ("en", "Philosophy", Some(1532)), // The Prince
        345 => ("en", "Horror", Some(1897)),   // Dracula
        2600 => ("en", "Fiction", Some(1869)), // War and Peace

        // More classics
        1080 => ("en", "Philosophy", Some(1729)), // A Modest Proposal
        16328 => ("en", "Poetry", Some(1000)),    // Beowulf
        768 => ("en", "Romance", Some(1847)),     // Wuthering Heights
        1400 => ("en", "Fiction", Some(1861)),    // Great Expectations
        174 => ("en", "Fiction", Some(1890)),     // Dorian Gray
        120 => ("en", "Adventure", Some(1883)),   // Treasure Island
        219 => ("en", "Fiction", Some(1899)),     // Heart of Darkness
        1260 => ("en", "Romance", Some(1847)),    // Jane Eyre
        5200 => ("de", "Fiction", Some(1915)),    // Metamorphosis (German origin)
        244 => ("en", "Mystery", Some(1887)),     // A Study in Scarlet
        1952 => ("en", "Fiction", Some(1892)),    // The Yellow Wallpaper
        76 => ("en", "Adventure", Some(1884)),    // Huckleberry Finn
        55 => ("en", "Children", Some(1900)),     // Wizard of Oz
        1184 => ("fr", "Adventure", Some(1844)),  // Count of Monte Cristo (French origin)
        4300 => ("en", "Fiction", Some(1922)),    // Ulysses
        28054 => ("ru", "Fiction", Some(1880)),   // Brothers Karamazov (Russian origin)
        2554 => ("ru", "Fiction", Some(1866)),    // Crime and Punishment
        36 => ("en", "Science Fiction", Some(1898)), // War of the Worlds
        35 => ("en", "Science Fiction", Some(1895)), // The Time Machine
        1934 => ("en", "Poetry", Some(1789)),     // Songs of Innocence
        158 => ("en", "Romance", Some(1815)),     // Emma
        161 => ("en", "Romance", Some(1811)),     // Sense and Sensibility
        105 => ("en", "Romance", Some(1817)),     // Persuasion
        145 => ("en", "Fiction", Some(1871)),     // Middlemarch
        1727 => ("el", "Poetry", Some(-700)),     // The Odyssey (Ancient Greek)
        6130 => ("el", "Poetry", Some(-750)),     // The Iliad
        1497 => ("el", "Philosophy", Some(-375)), // The Republic
        2009 => ("en", "Science", Some(1859)),    // Origin of Species
        4363 => ("de", "Philosophy", Some(1886)), // Beyond Good and Evil
        132 => ("zh", "Philosophy", Some(-500)),  // The Art of War (Chinese origin)
        996 => ("es", "Fiction", Some(1605)),     // Don Quixote (Spanish origin)
        1399 => ("ru", "Fiction", Some(1877)),    // Anna Karenina
        25344 => ("en", "Fiction", Some(1850)),   // The Scarlet Letter
        209 => ("en", "Horror", Some(1898)),      // Turn of the Screw
        113 => ("en", "Children", Some(1911)),    // The Secret Garden
        236 => ("en", "Children", Some(1894)),    // The Jungle Book
        1322 => ("en", "Poetry", Some(1855)),     // Leaves of Grass
        100 => ("en", "Drama", Some(1623)),       // Shakespeare Complete Works
        1251 => ("en", "Fiction", Some(1485)),    // Le Morte d'Arthur
        3600 => ("de", "Philosophy", Some(1883)), // Thus Spoke Zarathustra

        // Dickens
        730 => ("en", "Fiction", Some(1838)),  // Oliver Twist
        766 => ("en", "Fiction", Some(1850)),  // David Copperfield
        786 => ("en", "Fiction", Some(1854)),  // Hard Times
        1023 => ("en", "Fiction", Some(1853)), // Bleak House
        883 => ("en", "Fiction", Some(1857)),  // Little Dorrit
        564 => ("en", "Fiction", Some(1865)),  // Our Mutual Friend
        580 => ("en", "Fiction", Some(1837)),  // Pickwick Papers
        917 => ("en", "Fiction", Some(1841)),  // Old Curiosity Shop
        821 => ("en", "Fiction", Some(1848)),  // Dombey and Son
        653 => ("en", "Fiction", Some(1839)),  // Nicholas Nickleby

        // More Austen
        121 => ("en", "Romance", Some(1817)), // Northanger Abbey
        1212 => ("en", "Romance", Some(1871)), // Lady Susan
        946 => ("en", "Romance", Some(1814)), // Mansfield Park

        // American Literature
        45 => ("en", "Children", Some(1908)), // Anne of Green Gables
        205 => ("en", "Fiction", Some(1906)), // Walden
        4280 => ("en", "Poetry", Some(1855)), // Leaves of Grass
        31 => ("en", "Fiction", Some(1807)),  // Rip Van Winkle
        514 => ("en", "Fiction", Some(1899)), // Little Women
        3176 => ("en", "Fiction", Some(1899)), // Awakening

        // H.G. Wells
        159 => ("en", "Science Fiction", Some(1897)), // Invisible Man
        5230 => ("en", "Science Fiction", Some(1895)), // Time Machine
        1743 => ("en", "Science Fiction", Some(1904)), // Food of the Gods
        718 => ("en", "Science Fiction", Some(1896)), // Island of Dr Moreau

        // Dostoevsky
        600 => ("ru", "Fiction", Some(1866)), // Notes from Underground
        12150 => ("ru", "Fiction", Some(1868)), // The Idiot

        // Tolstoy
        243 => ("ru", "Fiction", Some(1877)),  // Anna Karenina
        2142 => ("ru", "Fiction", Some(1899)), // Resurrection
        689 => ("ru", "Fiction", Some(1886)),  // Death of Ivan Ilyich

        // French Literature
        135 => ("fr", "Fiction", Some(1862)),  // Les Misérables
        2413 => ("fr", "Fiction", Some(1844)), // Three Musketeers
        17989 => ("fr", "Fiction", Some(1844)), // Count of Monte Cristo
        804 => ("fr", "Science Fiction", Some(1870)), // Twenty Thousand Leagues
        164 => ("fr", "Science Fiction", Some(1873)), // Around the World in 80 Days
        103 => ("fr", "Science Fiction", Some(1865)), // From Earth to Moon

        // German Literature
        2229 => ("de", "Fiction", Some(1774)), // Sorrows of Young Werther
        2500 => ("de", "Fiction", Some(1912)), // Death in Venice

        // Philosophy
        5827 => ("de", "Philosophy", Some(1788)), // Critique of Pure Reason
        4705 => ("en", "Philosophy", Some(1651)), // Leviathan
        7370 => ("la", "Philosophy", Some(1677)), // Ethics (Spinoza)
        3207 => ("en", "Philosophy", Some(1859)), // On Liberty

        // Poetry
        1065 => ("en", "Poetry", Some(1667)),  // Paradise Lost
        22120 => ("en", "Poetry", Some(1320)), // Divine Comedy (English trans)

        // Mystery/Detective
        2852 => ("en", "Mystery", Some(1902)), // Hound of Baskervilles
        108 => ("en", "Mystery", Some(1893)),  // Adventures of Sherlock Holmes
        834 => ("en", "Mystery", Some(1894)),  // Memoirs of Sherlock Holmes

        // Multilingual Preset - French Literature
        17489 => ("fr", "Fiction", Some(1862)), // Les Misérables Tome I
        17490 => ("fr", "Fiction", Some(1862)), // Les Misérables Tome II
        17493 => ("fr", "Fiction", Some(1862)), // Les Misérables Tome III
        17494 => ("fr", "Fiction", Some(1862)), // Les Misérables Tome IV
        17518 => ("fr", "Fiction", Some(1862)), // Les Misérables Tome V
        5193 => ("fr", "Fiction", Some(1831)),  // Notre-Dame de Paris
        17519 => ("fr", "Fiction", Some(1866)), // Les Travailleurs de la mer
        13704 => ("fr", "Poetry", Some(1856)),  // Les Contemplations
        8356 => ("fr", "Fiction", Some(1844)),  // Le Comte de Monte-Cristo
        8357 => ("fr", "Fiction", Some(1844)),  // Le Comte de Monte-Cristo II
        2408 => ("fr", "Fiction", Some(1844)),  // Les Trois Mousquetaires
        965 => ("fr", "Science Fiction", Some(1870)), // Vingt mille lieues sous les mers
        4791 => ("fr", "Fiction", Some(1873)),  // Le Tour du monde en 80 jours
        4982 => ("fr", "Science Fiction", Some(1865)), // De la Terre à la Lune
        4657 => ("fr", "Science Fiction", Some(1864)), // Voyage au centre de la Terre
        2650 => ("fr", "Philosophy", Some(1759)), // Candide
        39855 => ("fr", "Fiction", Some(1913)), // À la recherche du temps perdu
        2596 => ("fr", "Poetry", Some(1857)),   // Les Fleurs du mal
        14658 => ("fr", "Fiction", Some(1856)), // Madame Bovary
        23085 => ("fr", "Fiction", Some(1869)), // L'Éducation sentimentale
        799 => ("fr", "Fiction", Some(1835)),   // Le Père Goriot
        1553 => ("fr", "Fiction", Some(1833)),  // Eugénie Grandet
        13159 => ("fr", "Fiction", Some(1885)), // Germinal
        5711 => ("fr", "Fiction", Some(1880)),  // Nana
        9909 => ("fr", "Drama", Some(1637)),    // Le Cid
        11120 => ("fr", "Drama", Some(1664)),   // Tartuffe
        5180 => ("fr", "Drama", Some(1670)),    // Le Bourgeois gentilhomme
        5428 => ("fr", "Drama", Some(1671)),    // Les Fourberies de Scapin

        // Multilingual Preset - German Literature
        2407 => ("de", "Drama", Some(1808)), // Faust: Eine Tragödie
        22382 => ("de", "Drama", Some(1832)), // Faust II
        7183 => ("de", "Fiction", Some(1925)), // Der Prozess (Kafka)
        22367 => ("de", "Fiction", Some(1926)), // Das Schloss
        7205 => ("de", "Philosophy", Some(1886)), // Jenseits von Gut und Böse
        5328 => ("de", "Philosophy", Some(1887)), // Zur Genealogie der Moral
        46799 => ("de", "Fiction", Some(1901)), // Buddenbrooks
        26657 => ("de", "Drama", Some(1781)), // Die Räuber
        21710 => ("de", "Drama", Some(1800)), // Maria Stuart
        6498 => ("de", "Drama", Some(1779)), // Nathan der Weise
        12176 => ("de", "Drama", Some(1772)), // Emilia Galotti
        6710 => ("de", "Fiction", Some(1895)), // Effi Briest

        // Multilingual Preset - Spanish Literature
        2000 => ("es", "Fiction", Some(1605)),  // Don Quijote
        5921 => ("es", "Fiction", Some(1613)),  // Novelas Ejemplares
        15353 => ("es", "Drama", Some(1499)),   // La Celestina
        10676 => ("es", "Fiction", Some(1554)), // El Lazarillo de Tormes
        15532 => ("es", "Drama", Some(1635)),   // La vida es sueño
        5658 => ("es", "Drama", Some(1844)),    // Don Juan Tenorio
        17147 => ("es", "Fiction", Some(1887)), // Fortunata y Jacinta
        49836 => ("es", "Fiction", Some(1876)), // Doña Perfecta
        36788 => ("es", "Fiction", Some(1874)), // Pepita Jiménez

        // Multilingual Preset - Italian Literature
        1000 => ("it", "Poetry", Some(1320)), // La Divina Commedia
        23700 => ("it", "Philosophy", Some(1532)), // Il Principe
        18909 => ("it", "Fiction", Some(1827)), // I Promessi Sposi
        19965 => ("it", "Poetry", Some(1516)), // Orlando Furioso
        30148 => ("it", "Poetry", Some(1581)), // Gerusalemme liberata
        6759 => ("it", "Poetry", Some(1374)), // Il Canzoniere

        // Multilingual Preset - Portuguese Literature
        3333 => ("pt", "Poetry", Some(1572)),   // Os Lusíadas
        16565 => ("pt", "Fiction", Some(1899)), // Dom Casmurro
        55752 => ("pt", "Fiction", Some(1881)), // Memórias Póstumas de Brás Cubas
        27964 => ("pt", "Fiction", Some(1878)), // O Primo Basílio

        // Multilingual Preset - Latin Classics
        227 => ("la", "Poetry", Some(8)), // Metamorphoses (Ovid)
        17173 => ("la", "History", Some(-50)), // Commentarii de Bello Gallico
        18269 => ("la", "Philosophy", Some(-54)), // De Re Publica
        19942 => ("la", "History", Some(116)), // Annales (Tacitus)
        2721 => ("la", "Poetry", Some(-19)), // Aeneis
        22117 => ("la", "Philosophy", Some(398)), // Confessiones (Augustine)

        // Default fallback for unknown books
        _ => ("en", "Fiction", None),
    };

    BookMetadata::new(id, title, author, metadata.0, metadata.1, metadata.2)
}

/// Look up book metadata by ID from known presets
/// Returns Some(BookMetadata) if the book is in any preset, None otherwise
pub fn lookup_book_metadata(id: u32) -> Option<BookMetadata> {
    // Check all presets for this book ID
    for preset in [
        BookPreset::Quick,
        BookPreset::Classics,
        BookPreset::Full,
        BookPreset::Massive,
        BookPreset::Multilingual,
    ] {
        for (book_id, title, author) in preset.books() {
            if book_id == id {
                return Some(enrich_book_metadata(id, title, author));
            }
        }
    }
    None
}

/// Get book metadata for an ID, falling back to generic metadata if unknown
pub fn get_book_metadata(id: u32) -> BookMetadata {
    lookup_book_metadata(id).unwrap_or_else(|| {
        BookMetadata::new(
            id,
            &format!("Book {}", id),
            "Unknown",
            "en",
            "Fiction",
            None,
        )
    })
}

impl std::str::FromStr for BookPreset {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "quick" => Ok(Self::Quick),
            "classics" => Ok(Self::Classics),
            "full" => Ok(Self::Full),
            "massive" => Ok(Self::Massive),
            "multilingual" => Ok(Self::Multilingual),
            "gutenberg" => Ok(Self::Gutenberg),
            _ => Err(format!("Unknown preset: {}. Use 'quick', 'classics', 'full', 'massive', 'multilingual', or 'gutenberg'.", s)),
        }
    }
}
