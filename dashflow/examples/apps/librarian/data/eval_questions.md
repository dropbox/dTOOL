# Librarian Evaluation Questions

Golden Q&A dataset for evaluating the Superhuman Librarian's search and retrieval capabilities.

## Pride and Prejudice

### Easy
- Q: What is Mr. Darcy's first name?
- A: Fitzwilliam
- Source: Chapter 61

- Q: How many daughters do Mr. and Mrs. Bennet have?
- A: Five
- Source: Chapter 1

- Q: Who does Jane Bennet marry?
- A: Mr. Bingley (Charles Bingley)
- Source: Chapter 55

### Medium
- Q: Why does Elizabeth initially dislike Mr. Darcy?
- A: He appears proud and refuses to dance with her at the ball, calling her "tolerable" but not handsome enough to tempt him
- Eval: semantic_similarity

- Q: What does Mr. Darcy's letter to Elizabeth reveal?
- A: It reveals that Wickham tried to elope with Darcy's sister Georgiana and explains Darcy's reasons for separating Jane and Bingley
- Eval: semantic_similarity

- Q: Why does Lady Catherine de Bourgh visit Elizabeth?
- A: To demand that Elizabeth promise never to accept an engagement to Mr. Darcy
- Eval: semantic_similarity

### Hard
- Q: How does Elizabeth's opinion of Darcy change throughout the novel?
- A: She initially despises him for his pride but gradually comes to respect and love him after reading his letter, visiting Pemberley, and seeing his kindness to her relatives
- Eval: llm_judge

## Moby Dick

### Easy
- Q: What is Captain Ahab's ship called?
- A: The Pequod
- Source: Chapter 16

- Q: Who is the narrator of Moby Dick?
- A: Ishmael
- Source: Chapter 1

- Q: What color is Moby Dick?
- A: White
- Source: Throughout

### Medium
- Q: Why does Captain Ahab hunt Moby Dick?
- A: The whale bit off his leg in a previous encounter, and Ahab has become obsessed with revenge
- Eval: semantic_similarity

- Q: Who is Queequeg?
- A: A harpooner from the South Pacific islands who becomes Ishmael's close friend and shares a bed with him at the inn
- Eval: semantic_similarity

- Q: What happens to the Pequod at the end?
- A: Moby Dick destroys the ship, killing everyone except Ishmael who survives by floating on a coffin
- Eval: semantic_similarity

### Hard
- Q: What does the white whale symbolize in the novel?
- A: Various interpretations including nature's indifference to man, the unknowable, obsession's destructive power, and the futility of challenging forces beyond human control
- Eval: llm_judge

## Frankenstein

### Easy
- Q: Who created Frankenstein's monster?
- A: Victor Frankenstein
- Source: Chapter 5

- Q: Where does Victor Frankenstein study?
- A: University of Ingolstadt
- Source: Chapter 3

- Q: Who narrates Victor's story to the reader?
- A: Robert Walton (through letters to his sister)
- Source: Letters

### Medium
- Q: Why does the creature kill William?
- A: He discovers William is a relation of Frankenstein and kills him out of revenge against his creator who abandoned him
- Eval: semantic_similarity

- Q: What does the creature ask Victor to create for him?
- A: A female companion so he won't be alone
- Eval: semantic_similarity

- Q: Why does Victor destroy the female creature?
- A: He fears they would breed and create a race of monsters, or that she might reject the original creature
- Eval: semantic_similarity

### Hard
- Q: How does Shelley explore the theme of scientific responsibility?
- A: Through Victor's failure to consider the consequences of his creation, his abandonment of the creature, and the resulting chain of destruction, Shelley warns against unchecked scientific ambition
- Eval: llm_judge

## Hamlet

### Easy
- Q: Who is Hamlet's father's killer?
- A: Claudius (his uncle)
- Source: Act 1, Scene 5

- Q: What does Hamlet see that reveals the truth about his father's death?
- A: His father's ghost
- Source: Act 1, Scene 5

- Q: Who does Hamlet accidentally kill thinking it was Claudius?
- A: Polonius
- Source: Act 3, Scene 4

### Medium
- Q: What is Hamlet's famous soliloquy about?
- A: Whether to live or die ("To be or not to be"); contemplating existence and whether it's nobler to endure suffering or to take action against it
- Eval: semantic_similarity

- Q: Why does Ophelia go mad?
- A: Due to her father Polonius's death at Hamlet's hands and Hamlet's cruel treatment of her
- Eval: semantic_similarity

- Q: How does Hamlet test Claudius's guilt?
- A: By having actors perform a play ("The Murder of Gonzago" or "The Mousetrap") that mirrors his father's murder
- Eval: semantic_similarity

### Hard
- Q: What role does delay and inaction play in Hamlet's tragedy?
- A: Hamlet's hesitation to kill Claudius, his need for proof, his philosophical doubts, and his desire for perfect revenge all contribute to the escalating death toll including his own
- Eval: llm_judge

## A Tale of Two Cities

### Easy
- Q: What are the two cities in the title?
- A: London and Paris
- Source: Book 1

- Q: Who is Sydney Carton?
- A: A dissolute English lawyer who resembles Charles Darnay and eventually sacrifices himself for him
- Source: Throughout

- Q: What is the famous opening line?
- A: "It was the best of times, it was the worst of times"
- Source: Book 1, Chapter 1

### Medium
- Q: Why was Doctor Manette imprisoned in the Bastille?
- A: For witnessing crimes committed by the Evremonde family (Darnay's relatives) and trying to report them
- Eval: semantic_similarity

- Q: What does Sydney Carton do at the end of the novel?
- A: He takes Charles Darnay's place at the guillotine, sacrificing himself so Darnay can live with Lucie
- Eval: semantic_similarity

- Q: Why does Madame Defarge want to destroy the Evremonde family?
- A: Her family was destroyed by the Evremondes; her sister was raped and her brother killed by them
- Eval: semantic_similarity

### Hard
- Q: How does Dickens contrast the themes of resurrection and sacrifice?
- A: Doctor Manette is "recalled to life" from prison, Carton finds spiritual redemption through his sacrifice, and the revolution offers both death and the promise of renewal for society
- Eval: llm_judge

## Alice's Adventures in Wonderland

### Easy
- Q: What does Alice follow down the rabbit hole?
- A: The White Rabbit
- Source: Chapter 1

- Q: What causes Alice to grow and shrink?
- A: Eating and drinking various things (cakes, drinks, mushroom pieces)
- Source: Chapters 1, 4, 5

- Q: What game does the Queen of Hearts play?
- A: Croquet (with flamingos as mallets and hedgehogs as balls)
- Source: Chapter 8

### Medium
- Q: Who tells Alice "We're all mad here"?
- A: The Cheshire Cat
- Eval: exact_match

- Q: What happens at the Mad Tea Party?
- A: Alice joins the Mad Hatter, March Hare, and Dormouse for a tea party where they ask riddles, change seats constantly, and the Dormouse tells a story about treacle
- Eval: semantic_similarity

- Q: Why is the Queen of Hearts always shouting "Off with their heads"?
- A: She uses execution as her default punishment for any offense, though as the King notes, no one is actually ever executed
- Eval: semantic_similarity

### Hard
- Q: How does Carroll use nonsense to satirize Victorian society?
- A: Through the arbitrary rules of Wonderland, the meaningless court trial, and the absurd hierarchies, Carroll critiques Victorian social conventions, education, and authority
- Eval: llm_judge

## Dracula

### Easy
- Q: Who is the narrator at the beginning of Dracula?
- A: Jonathan Harker
- Source: Jonathan Harker's Journal

- Q: Where is Count Dracula's castle located?
- A: Transylvania (in the Carpathian Mountains)
- Source: Chapter 1

- Q: Who leads the hunt against Dracula?
- A: Professor Abraham Van Helsing
- Source: Throughout

### Medium
- Q: How is Lucy Westenra transformed?
- A: Dracula drains her blood over multiple nights while she sleepwalks, and despite blood transfusions, she dies and becomes a vampire
- Eval: semantic_similarity

- Q: How can Dracula be killed?
- A: By driving a wooden stake through his heart, cutting off his head, and filling his mouth with garlic (or exposure to sunlight)
- Eval: semantic_similarity

- Q: What powers does Dracula possess?
- A: He can control wolves and weather, transform into a bat or mist, has superhuman strength, doesn't age, and can create other vampires
- Eval: semantic_similarity

### Hard
- Q: How does Stoker use the epistolary format to build tension?
- A: The novel's diary entries, letters, and newspaper clippings create multiple perspectives and dramatic irony, as readers often know more than individual characters
- Eval: llm_judge

## The Adventures of Sherlock Holmes

### Easy
- Q: Where does Sherlock Holmes live?
- A: 221B Baker Street, London
- Source: Throughout

- Q: Who is Holmes's companion and chronicler?
- A: Doctor John Watson
- Source: Throughout

- Q: What is Holmes's famous catchphrase about deduction?
- A: "Elementary, my dear Watson" (though this exact phrase doesn't appear in the original stories)
- Eval: semantic_similarity

### Medium
- Q: What is Holmes's attitude toward emotions and love?
- A: He considers them distractions from pure logic and claims to be immune to them, though he admires Irene Adler ("The Woman")
- Eval: semantic_similarity

- Q: What are Holmes's vices?
- A: Cocaine use, tobacco (pipe smoking), and periods of depression/boredom between cases
- Eval: semantic_similarity

- Q: How does Holmes describe his method of reasoning?
- A: He observes small details others overlook and uses deduction to eliminate impossible explanations, leaving whatever remains, however improbable, as the truth
- Eval: semantic_similarity

### Hard
- Q: How does Doyle use the Holmes stories to comment on Victorian society?
- A: Through cases involving class differences, domestic violence, colonialism, and social hypocrisy, Holmes often protects individuals against unjust social systems
- Eval: llm_judge

## Wuthering Heights

### Easy
- Q: Who is the main narrator of the inner story?
- A: Nelly Dean (Ellen Dean)
- Source: Throughout

- Q: What is the name of the Earnshaw family home?
- A: Wuthering Heights
- Source: Chapter 1

- Q: Who does Catherine Earnshaw marry?
- A: Edgar Linton
- Source: Chapter 9

### Medium
- Q: Why does Heathcliff leave Wuthering Heights?
- A: He overhears Catherine say it would degrade her to marry him, though she also says she loves him
- Eval: semantic_similarity

- Q: How does Heathcliff get his revenge on the Earnshaws and Lintons?
- A: He marries Isabella Linton, gambles Hindley into debt to take Wuthering Heights, and forces young Cathy to marry his son Linton to gain Thrushcross Grange
- Eval: semantic_similarity

- Q: What happens to Heathcliff at the end?
- A: He becomes obsessed with seeing Catherine's ghost, stops eating, and dies, apparently happily, to be buried next to her
- Eval: semantic_similarity

### Hard
- Q: How does Brontë subvert the romantic hero archetype with Heathcliff?
- A: Heathcliff begins as a sympathetic orphan but becomes cruel and vengeful, showing how abuse and social rejection can destroy a person, challenging simple romantic idealization
- Eval: llm_judge

## Great Expectations

### Easy
- Q: What is Pip's full name?
- A: Philip Pirrip
- Source: Chapter 1

- Q: What does Miss Havisham always wear?
- A: Her old wedding dress
- Source: Chapter 8

- Q: Who is Pip's secret benefactor?
- A: Magwitch (Abel Magwitch), the convict Pip helped as a child
- Source: Chapter 39

### Medium
- Q: Why does Miss Havisham raise Estella?
- A: To use her as an instrument of revenge against men, teaching her to break hearts as hers was broken
- Eval: semantic_similarity

- Q: What happens to Miss Havisham?
- A: She catches fire from a candle and dies from her burns after asking Pip's forgiveness
- Eval: semantic_similarity

- Q: How does Pip change after learning his benefactor's identity?
- A: He's horrified that his "expectations" come from a convict, not Miss Havisham, but gradually comes to appreciate Magwitch's sacrifice
- Eval: semantic_similarity

### Hard
- Q: How does Dickens critique class aspiration in Great Expectations?
- A: Pip's desire to become a gentleman brings him shame for his origins, snobbery toward Joe, and ultimately unhappiness, while true nobility comes from characters like Joe and Magwitch
- Eval: llm_judge

## Jane Eyre

### Easy
- Q: Where does Jane first meet Mr. Rochester?
- A: On the road near Thornfield, when his horse slips on ice
- Source: Chapter 12

- Q: Who is the "madwoman in the attic"?
- A: Bertha Mason, Rochester's first wife
- Source: Chapter 26

- Q: Where does Jane become a teacher?
- A: Lowood School (as a student first, then teacher)
- Source: Chapters 5-10

### Medium
- Q: Why can't Jane marry Rochester initially?
- A: He is already married to Bertha Mason, who is kept in the attic at Thornfield
- Eval: semantic_similarity

- Q: What does Jane inherit from her uncle?
- A: Twenty thousand pounds, which she divides equally with her cousins Diana, Mary, and St. John Rivers
- Eval: semantic_similarity

- Q: How does Jane hear Rochester calling her?
- A: Through a supernatural experience - she hears him calling her name across the moors from miles away
- Eval: semantic_similarity

### Hard
- Q: How does Brontë address female independence and equality?
- A: Jane insists on equality with Rochester despite class differences, refuses to be a mistress, leaves when her integrity is threatened, and returns only as his equal
- Eval: llm_judge

## The Picture of Dorian Gray

### Easy
- Q: What happens to Dorian Gray's portrait?
- A: It ages and shows his sins while Dorian remains young and beautiful
- Source: Chapter 7

- Q: Who paints Dorian's portrait?
- A: Basil Hallward
- Source: Chapter 1

- Q: Who is the negative influence on Dorian?
- A: Lord Henry Wotton (Harry)
- Source: Throughout

### Medium
- Q: What happens to Sibyl Vane?
- A: She kills herself after Dorian cruelly rejects her when her acting becomes sincere rather than theatrical
- Eval: semantic_similarity

- Q: How does Dorian murder Basil?
- A: He stabs him in the neck after Basil sees the corrupted portrait
- Eval: semantic_similarity

- Q: What happens when Dorian stabs the portrait?
- A: He dies instantly, becoming old and withered, while the portrait returns to its original beauty
- Eval: semantic_similarity

### Hard
- Q: How does Wilde use the portrait to explore morality and aestheticism?
- A: The portrait represents the separation of art from morality that aestheticism proposed, but its corruption shows the impossibility of escaping moral consequences
- Eval: llm_judge

## Crime and Punishment

### Easy
- Q: What crime does Raskolnikov commit?
- A: He murders the pawnbroker Alyona Ivanovna (and her sister Lizaveta)
- Source: Part 1, Chapter 7

- Q: Who is the detective investigating Raskolnikov?
- A: Porfiry Petrovich
- Source: Part 3

- Q: Who does Raskolnikov confess to first?
- A: Sonya Marmeladov
- Source: Part 5, Chapter 4

### Medium
- Q: What is Raskolnikov's theory about extraordinary men?
- A: He believes extraordinary men (like Napoleon) have the right to transgress moral laws if it serves a higher purpose
- Eval: semantic_similarity

- Q: Why does Raskolnikov confess to the police?
- A: Sonya convinces him that suffering and confession are the path to redemption, and he cannot bear his psychological torment
- Eval: semantic_similarity

- Q: What happens to Raskolnikov at the end?
- A: He is sentenced to eight years in Siberia, where Sonya follows him and he finally experiences genuine repentance and love
- Eval: semantic_similarity

### Hard
- Q: How does Dostoevsky critique utilitarianism and nihilism?
- A: Raskolnikov's murder committed for "rational" reasons leads to psychological destruction, showing that purely intellectual morality divorced from conscience is untenable
- Eval: llm_judge

## The Brothers Karamazov

### Easy
- Q: Who are the three Karamazov brothers?
- A: Dmitri (Mitya), Ivan, and Alexei (Alyosha)
- Source: Book 1

- Q: Who is murdered in the novel?
- A: Fyodor Pavlovich Karamazov (the brothers' father)
- Source: Book 8

- Q: Which brother becomes a monk?
- A: Alyosha (Alexei)
- Source: Book 1

### Medium
- Q: What is Ivan's story about the Grand Inquisitor?
- A: A poem where Jesus returns during the Spanish Inquisition and is arrested; the Grand Inquisitor argues humanity prefers security to freedom
- Eval: semantic_similarity

- Q: Who actually murdered Fyodor?
- A: Smerdyakov, the illegitimate son/servant, partly inspired by Ivan's ideas
- Eval: semantic_similarity

- Q: What happens to Dmitri?
- A: He is convicted of the murder despite being innocent and sentenced to Siberia
- Eval: semantic_similarity

### Hard
- Q: How does Dostoevsky explore the problem of theodicy?
- A: Through Ivan's rebellion against God who allows children to suffer, and Alyosha's faith that accepts mystery, Dostoevsky wrestles with faith's compatibility with evil
- Eval: llm_judge

## War and Peace

### Easy
- Q: What historical event forms the backdrop of War and Peace?
- A: The Napoleonic Wars, particularly Napoleon's invasion of Russia in 1812
- Source: Throughout

- Q: Who are the main aristocratic families in the novel?
- A: The Rostovs, Bolkonskys, and Bezukhovs
- Source: Throughout

- Q: Who does Natasha Rostova eventually marry?
- A: Pierre Bezukhov
- Source: Epilogue

### Medium
- Q: How is Pierre Bezukhov transformed by his experiences?
- A: He moves from aimless wealth through failed marriage, Freemasonry, the Battle of Borodino, and captivity to find meaning in simple living and family
- Eval: semantic_similarity

- Q: What happens to Prince Andrei?
- A: He is wounded at Borodino, reconciles with Natasha, and dies at peace with his former fiancée at his side
- Eval: semantic_similarity

- Q: What is Tolstoy's view of history as expressed in the novel?
- A: He argues that history is not driven by great men but by countless individual actions, and that Napoleon's supposed genius was actually circumstance
- Eval: semantic_similarity

### Hard
- Q: How does Tolstoy critique the concept of historical causation?
- A: Through philosophical digressions, he argues that historians falsely attribute events to individual decisions while real history emerges from millions of human wills and circumstances
- Eval: llm_judge

## Anna Karenina

### Easy
- Q: What is the famous opening line of Anna Karenina?
- A: "Happy families are all alike; every unhappy family is unhappy in its own way"
- Source: Part 1, Chapter 1

- Q: With whom does Anna have an affair?
- A: Count Alexei Vronsky
- Source: Part 2

- Q: How does Anna die?
- A: She throws herself under a train
- Source: Part 7, Chapter 31

### Medium
- Q: What does Levin's story represent as a counterpoint to Anna's?
- A: Levin finds meaning through work, family, and eventually faith, contrasting with Anna's destructive passion
- Eval: semantic_similarity

- Q: Why does Anna become increasingly paranoid?
- A: Social ostracism, Vronsky's continued freedom in society, her separation from her son, and morphine addiction lead to jealousy and despair
- Eval: semantic_similarity

- Q: How does society treat Anna versus Vronsky?
- A: Society punishes Anna with exclusion while largely forgiving Vronsky, highlighting Victorian double standards
- Eval: semantic_similarity

### Hard
- Q: How does Tolstoy explore the conflict between passion and duty?
- A: Anna's passion destroys her family and ultimately herself, while Levin's commitment to duty and faith brings fulfillment, though Tolstoy shows sympathy for Anna's situation
- Eval: llm_judge

## The Count of Monte Cristo

### Easy
- Q: Why is Edmond Dantès imprisoned?
- A: He is falsely accused of being a Bonapartist traitor
- Source: Chapters 5-7

- Q: Where is Dantès imprisoned?
- A: The Château d'If (an island fortress)
- Source: Chapter 7

- Q: Who teaches Dantès in prison?
- A: Abbé Faria
- Source: Chapters 14-17

### Medium
- Q: How does Dantès escape from prison?
- A: He switches places with the dead Abbé Faria in his burial sack and is thrown into the sea, from which he escapes
- Eval: semantic_similarity

- Q: Who are the three main villains Dantès seeks revenge against?
- A: Fernand (who stole his fiancée), Danglars (who wrote the accusation), and Villefort (who prosecuted him knowing his innocence)
- Eval: semantic_similarity

- Q: How does Monte Cristo ultimately punish his enemies?
- A: He ruins Danglars financially, exposes Fernand's treachery leading to his suicide, and reveals Villefort's crimes leading to his madness
- Eval: semantic_similarity

### Hard
- Q: How does Dumas explore the limits of revenge?
- A: Monte Cristo's revenge destroys innocents along with the guilty, and he comes to question whether he was right to assume God's role in dispensing justice
- Eval: llm_judge

## Les Misérables

### Easy
- Q: What crime was Jean Valjean originally imprisoned for?
- A: Stealing a loaf of bread
- Source: Book 2

- Q: Who relentlessly pursues Jean Valjean?
- A: Inspector Javert
- Source: Throughout

- Q: What does Valjean steal from the bishop?
- A: Silver candlesticks (and other silverware)
- Source: Book 1, Chapter 12

### Medium
- Q: Who is Cosette and how does Valjean come to care for her?
- A: Cosette is Fantine's daughter, left with the abusive Thénardiers. After Fantine's death, Valjean rescues and raises her
- Eval: semantic_similarity

- Q: What happens to Javert at the end?
- A: Unable to reconcile his duty with his debt to Valjean, who saved his life, Javert commits suicide by throwing himself into the Seine
- Eval: semantic_similarity

- Q: What role does the 1832 Paris uprising play?
- A: Young revolutionaries including Marius build a barricade; Valjean saves Marius, and most insurgents die, including young Gavroche
- Eval: semantic_similarity

### Hard
- Q: How does Hugo use the novel to advocate for social reform?
- A: Through showing how poverty, injustice, and lack of education create crime and suffering, Hugo argues for compassion and systemic change over punishment
- Eval: llm_judge

## The Odyssey

### Easy
- Q: Who is trying to return home in The Odyssey?
- A: Odysseus (Ulysses in Latin)
- Source: Throughout

- Q: How long is Odysseus away from home?
- A: 20 years (10 years of war, 10 years of wandering)
- Source: Throughout

- Q: Who is Odysseus's wife?
- A: Penelope
- Source: Throughout

### Medium
- Q: How does Odysseus escape the Cyclops?
- A: He blinds Polyphemus with a heated stake and escapes by clinging to the underside of sheep
- Eval: semantic_similarity

- Q: Why does it take Odysseus so long to return home?
- A: He offends Poseidon by blinding his son Polyphemus, encounters monsters, and is detained by Calypso and Circe
- Eval: semantic_similarity

- Q: How does Odysseus prove his identity when he returns?
- A: He strings his great bow that no suitor can string and shoots an arrow through twelve axe handles
- Eval: semantic_similarity

### Hard
- Q: What does Odysseus's journey reveal about Greek values?
- A: It celebrates cunning (metis) alongside strength, loyalty to homeland, hospitality (xenia), and the importance of returning to restore proper order
- Eval: llm_judge

## The Iliad

### Easy
- Q: What is the central conflict of The Iliad?
- A: The quarrel between Achilles and Agamemnon during the Trojan War
- Source: Book 1

- Q: Why is Achilles angry at Agamemnon?
- A: Agamemnon takes Briseis, Achilles' war prize, after being forced to give up Chryseis
- Source: Book 1

- Q: What happens to Hector?
- A: Achilles kills him in combat and drags his body behind his chariot
- Source: Book 22

### Medium
- Q: Why does Achilles return to battle?
- A: Patroclus, his companion, is killed by Hector while wearing Achilles' armor
- Eval: semantic_similarity

- Q: What is Achilles' tragic flaw?
- A: His excessive pride and wrath (menis), which causes him to withdraw from battle, leading to the deaths of many Greeks including Patroclus
- Eval: semantic_similarity

- Q: How does the Iliad end?
- A: Priam comes to ransom Hector's body; Achilles shows mercy and returns it, and both sides observe a funeral truce
- Eval: semantic_similarity

### Hard
- Q: How does Homer complicate the concept of heroism?
- A: By showing the costs of Achilles' glory (Patroclus's death, his own fated death), the humanity of enemies like Hector, and the suffering war brings to both sides
- Eval: llm_judge

## The Republic (Plato)

### Easy
- Q: What is the central question of The Republic?
- A: What is justice, and is the just life happier than the unjust life?
- Source: Book 1

- Q: What is Plato's Allegory of the Cave about?
- A: Prisoners in a cave see only shadows and mistake them for reality; the philosopher escapes to see the sun (truth)
- Source: Book 7

- Q: What are the three parts of the soul according to Plato?
- A: Reason, spirit (thumos), and appetite
- Source: Book 4

### Medium
- Q: What are the three classes in Plato's ideal city?
- A: Rulers (philosopher-kings), guardians (warriors), and producers (craftsmen, farmers)
- Eval: semantic_similarity

- Q: Why does Plato advocate for philosopher-kings?
- A: Only those who know the Forms (especially the Form of the Good) can rule wisely; rulers need philosophical knowledge, not just power
- Eval: semantic_similarity

- Q: What is Plato's view of poetry and art?
- A: He criticizes art as imitation twice removed from truth and advocates censoring poetry that portrays gods or heroes badly
- Eval: semantic_similarity

### Hard
- Q: How does Plato's tripartite soul theory justify his political hierarchy?
- A: Just as the soul is healthy when reason rules spirit and appetite, the city is just when philosophers rule warriors and producers, each doing their proper function
- Eval: llm_judge

## Don Quixote

### Easy
- Q: What does Don Quixote believe himself to be?
- A: A knight-errant
- Source: Part 1, Chapter 1

- Q: Who is Don Quixote's squire?
- A: Sancho Panza
- Source: Part 1, Chapter 7

- Q: What does Don Quixote attack, thinking they are giants?
- A: Windmills
- Source: Part 1, Chapter 8

### Medium
- Q: Who is Dulcinea?
- A: A peasant woman (Aldonza Lorenzo) whom Don Quixote imagines as a noble lady and dedicates his deeds to
- Eval: semantic_similarity

- Q: How does Don Quixote's madness differ in Part 2?
- A: He becomes famous from Part 1 and encounters people who know his story and play along with or manipulate his delusions
- Eval: semantic_similarity

- Q: What happens to Don Quixote at the end?
- A: He renounces his knightly delusions, recovers his sanity, and dies peacefully in bed
- Eval: semantic_similarity

### Hard
- Q: How does Cervantes use Don Quixote to explore the nature of reality and fiction?
- A: Through characters who read the first part, fake enchantments, and debates about truth, Cervantes questions the boundaries between reality, fiction, and madness
- Eval: llm_judge

## Paradise Lost

### Easy
- Q: Who is the protagonist of Paradise Lost?
- A: Satan (or arguably Adam and Eve)
- Source: Books 1-2

- Q: What is Satan's famous line about ruling in Hell?
- A: "Better to reign in Hell than serve in Heaven"
- Source: Book 1, line 263

- Q: What fruit do Adam and Eve eat?
- A: The fruit of the Tree of Knowledge (traditionally an apple, though not specified)
- Source: Book 9

### Medium
- Q: Why does Satan rebel against God?
- A: Pride and envy, particularly at God's elevation of the Son
- Eval: semantic_similarity

- Q: How does Milton portray Satan sympathetically?
- A: Satan's speeches are eloquent and his defiance heroic, making him a complex figure rather than pure evil
- Eval: semantic_similarity

- Q: What is Milton's purpose in writing Paradise Lost?
- A: "To justify the ways of God to men" - to explain why God allows evil and free will
- Eval: semantic_similarity

### Hard
- Q: How does Milton reconcile free will with divine foreknowledge?
- A: God foresees but does not cause the Fall; Adam and Eve have true choice, and their freedom is what makes their obedience meaningful and their fall culpable
- Eval: llm_judge

## Beowulf

### Easy
- Q: What monster does Beowulf first fight?
- A: Grendel
- Source: Lines 700-836

- Q: Who rules the Danes when Beowulf arrives?
- A: King Hrothgar
- Source: Throughout

- Q: What kills Beowulf at the end?
- A: A dragon (with help from Wiglaf)
- Source: Lines 2500-2891

### Medium
- Q: How does Beowulf kill Grendel?
- A: He tears off Grendel's arm with his bare hands
- Eval: semantic_similarity

- Q: Why does Grendel's mother attack?
- A: To avenge her son's death
- Eval: semantic_similarity

- Q: What theme does the ending of Beowulf emphasize?
- A: The inevitability of death even for heroes, and the passing of the heroic age
- Eval: semantic_similarity

### Hard
- Q: How does Beowulf blend pagan and Christian elements?
- A: It celebrates pagan warrior values like loyalty and fame while incorporating Christian concepts of God, fate, and the dangers of pride
- Eval: llm_judge

## The Divine Comedy

### Easy
- Q: Who guides Dante through Hell?
- A: Virgil
- Source: Inferno, Canto 1

- Q: What are the three parts of The Divine Comedy?
- A: Inferno (Hell), Purgatorio (Purgatory), Paradiso (Paradise)
- Source: Structure

- Q: Who guides Dante through Paradise?
- A: Beatrice
- Source: Purgatorio, Canto 30

### Medium
- Q: What is the structure of Dante's Hell?
- A: Nine circles of increasing severity, organized by the type and gravity of sin, with Satan at the center
- Eval: semantic_similarity

- Q: What sinners are in the lowest circle of Hell?
- A: Traitors, frozen in ice, including Judas, Brutus, and Cassius being chewed by Satan
- Eval: semantic_similarity

- Q: Why is Dante's journey necessary?
- A: He has lost his way in life ("the dark wood") and must see the consequences of sin and the path to God to find redemption
- Eval: semantic_similarity

### Hard
- Q: How does Dante use contrapasso (poetic justice) in the Inferno?
- A: Each punishment ironically mirrors the sin: the lustful are blown by winds like passion, the flatterers swim in excrement, the fortune-tellers have heads on backwards
- Eval: llm_judge

## The Canterbury Tales

### Easy
- Q: Where are the pilgrims traveling to?
- A: Canterbury Cathedral (to the shrine of Thomas Becket)
- Source: General Prologue

- Q: Who is the host who proposes the storytelling contest?
- A: Harry Bailey (the Host of the Tabard Inn)
- Source: General Prologue

- Q: How many pilgrims are there in total?
- A: 29 (plus Chaucer himself and the Host)
- Source: General Prologue

### Medium
- Q: What is the Wife of Bath's main argument?
- A: Women should have sovereignty (control) in marriage
- Eval: semantic_similarity

- Q: How does the Pardoner's tale criticize him?
- A: He admits his relics are fake and he preaches against greed while being greedy, then tries to sell indulgences to the pilgrims
- Eval: semantic_similarity

- Q: What is the Knight's Tale about?
- A: Two knights, Palamon and Arcite, compete for the love of Emily; Arcite wins the duel but dies, and Palamon marries Emily
- Eval: semantic_similarity

### Hard
- Q: How does Chaucer use the frame narrative to create social commentary?
- A: The pilgrimage mixes all social classes, and their tales reveal hypocrisy, competing worldviews, and class tensions in medieval society
- Eval: llm_judge

## Middlemarch

### Easy
- Q: What does Dorothea Brooke want to accomplish?
- A: She wants to do something meaningful with her life, initially through supporting scholarly work
- Source: Book 1

- Q: Who does Dorothea marry first?
- A: Edward Casaubon
- Source: Book 1, Chapter 5

- Q: What is Lydgate's profession?
- A: Doctor/physician
- Source: Book 2

### Medium
- Q: Why is Dorothea's marriage to Casaubon unhappy?
- A: Casaubon is cold, jealous, and his scholarly work is futile; Dorothea realizes she cannot help him or find purpose through him
- Eval: semantic_similarity

- Q: How does Rosamond Vincy affect Lydgate's career?
- A: Her extravagance and social ambition lead to debt that compromises his medical independence and ideals
- Eval: semantic_similarity

- Q: Who does Dorothea marry after Casaubon dies?
- A: Will Ladislaw, giving up Casaubon's estate as required by his will
- Eval: semantic_similarity

### Hard
- Q: How does Eliot explore the theme of vocation and purpose?
- A: Through characters like Dorothea, Lydgate, and Farebrother, she shows how social constraints and personal weaknesses can thwart idealistic ambitions
- Eval: llm_judge

## The Scarlet Letter

### Easy
- Q: What does the scarlet letter "A" stand for?
- A: Adultery
- Source: Chapter 2

- Q: Who is Hester Prynne's partner in adultery?
- A: Arthur Dimmesdale
- Source: Chapter 23

- Q: Who is Roger Chillingworth's real relationship to Hester?
- A: He is her husband
- Source: Chapter 4

### Medium
- Q: Why won't Hester reveal her lover's identity?
- A: To protect him from public shame and because she alone chose to take full responsibility
- Eval: semantic_similarity

- Q: How does Chillingworth exact revenge?
- A: He poses as Dimmesdale's doctor to torment him psychologically, exacerbating his guilt
- Eval: semantic_similarity

- Q: What does Pearl represent in the novel?
- A: She is a living symbol of Hester's sin, but also of her passion and vitality, and ultimately of redemption
- Eval: semantic_similarity

### Hard
- Q: How does Hawthorne critique Puritan society?
- A: By showing how its harsh morality punishes Hester while Dimmesdale hides, and how private sin (Chillingworth's revenge) can be worse than public shame
- Eval: llm_judge

## Treasure Island

### Easy
- Q: Who is the narrator of Treasure Island?
- A: Jim Hawkins
- Source: Chapter 1

- Q: What is the name of the pirate ship?
- A: The Hispaniola
- Source: Chapter 7

- Q: What does Long John Silver pretend to be at the start?
- A: A cook (ship's cook)
- Source: Chapter 8

### Medium
- Q: How does Jim discover the pirates' plot?
- A: He hides in an apple barrel and overhears Silver planning mutiny
- Eval: semantic_similarity

- Q: Who was the original owner of the treasure?
- A: Captain Flint
- Eval: semantic_similarity

- Q: What happens to Long John Silver at the end?
- A: He escapes with some treasure and is never seen again
- Eval: semantic_similarity

### Hard
- Q: How does Stevenson create moral ambiguity with Long John Silver?
- A: Silver is charming, intelligent, and sometimes protective of Jim, making him sympathetic despite being a villain
- Eval: llm_judge

## Heart of Darkness

### Easy
- Q: Who narrates the story of Kurtz?
- A: Marlow
- Source: Frame narrative

- Q: Where does Kurtz run his ivory station?
- A: In the interior of the Belgian Congo
- Source: Throughout

- Q: What are Kurtz's famous last words?
- A: "The horror! The horror!"
- Source: Near the end

### Medium
- Q: What has Kurtz become in the Congo?
- A: A god-like figure to the natives who worship him; he has abandoned European civilization for brutal power
- Eval: semantic_similarity

- Q: What does Marlow tell Kurtz's fiancée?
- A: He lies, telling her that Kurtz's last words were her name
- Eval: semantic_similarity

- Q: What does the journey upriver symbolize?
- A: A journey into the darkness of human nature and colonialism
- Eval: semantic_similarity

### Hard
- Q: How does Conrad critique European colonialism?
- A: By showing its pretense of civilization masking brutal exploitation, and how it corrupts Europeans like Kurtz while destroying African peoples
- Eval: llm_judge

## The Origin of Species

### Easy
- Q: What theory does Darwin propose?
- A: Evolution by natural selection
- Source: Throughout

- Q: What inspired Darwin's theory?
- A: His observations during the voyage of the HMS Beagle, especially in the Galápagos Islands
- Source: Introduction

- Q: What does "survival of the fittest" mean?
- A: Organisms better adapted to their environment are more likely to survive and reproduce
- Source: Throughout

### Medium
- Q: How does natural selection work?
- A: Variation exists in populations; those with advantageous traits survive and reproduce more; over time this changes populations
- Eval: semantic_similarity

- Q: What evidence does Darwin use for evolution?
- A: Fossil record, geographic distribution, embryology, vestigial organs, artificial selection in breeding
- Eval: semantic_similarity

- Q: Why was Darwin's theory controversial?
- A: It contradicted literal Biblical creation, suggested humans and apes share ancestry, and removed the need for design
- Eval: semantic_similarity

### Hard
- Q: How does Darwin address the problem of complex organs like the eye?
- A: He argues they evolved gradually through intermediate stages, each of which provided some adaptive advantage
- Eval: llm_judge

## The Communist Manifesto

### Easy
- Q: Who wrote The Communist Manifesto?
- A: Karl Marx and Friedrich Engels
- Source: Title page

- Q: What is the famous opening line?
- A: "A spectre is haunting Europe—the spectre of communism"
- Source: Opening

- Q: What class will overthrow the bourgeoisie according to Marx?
- A: The proletariat (working class)
- Source: Throughout

### Medium
- Q: What is the bourgeoisie in Marxist theory?
- A: The capitalist class that owns the means of production
- Eval: semantic_similarity

- Q: What does Marx mean by "class struggle"?
- A: That history is driven by conflict between ruling classes and oppressed classes
- Eval: semantic_similarity

- Q: What is the end goal of communism according to Marx?
- A: A classless society where the means of production are commonly owned and the state "withers away"
- Eval: semantic_similarity

### Hard
- Q: How does Marx view capitalism's historical role?
- A: As a progressive force that destroyed feudalism and created unprecedented productive capacity, but which must be superseded by socialism
- Eval: llm_judge

## Walden

### Easy
- Q: Where did Thoreau live while writing Walden?
- A: In a cabin by Walden Pond
- Source: Throughout

- Q: How long did Thoreau live at Walden Pond?
- A: Two years, two months, and two days
- Source: Throughout

- Q: Who owned the land where Thoreau built his cabin?
- A: Ralph Waldo Emerson
- Source: Chapter 1

### Medium
- Q: What is Thoreau's purpose in living simply?
- A: To strip away superfluities and discover what is essential to life
- Eval: semantic_similarity

- Q: What does Thoreau mean by "quiet desperation"?
- A: That most people live unfulfilling lives conforming to society's expectations
- Eval: semantic_similarity

- Q: How does Thoreau view technology and progress?
- A: Skeptically—he questions whether the telegraph and railroad improve our lives or just speed up what isn't worth doing
- Eval: semantic_similarity

### Hard
- Q: How does Walden balance individualism with social critique?
- A: Thoreau advocates personal withdrawal for self-discovery while critiquing slavery, materialism, and conformity
- Eval: llm_judge

## Civil Disobedience

### Easy
- Q: What action prompted Thoreau to write this essay?
- A: His arrest for refusing to pay a poll tax
- Source: Throughout

- Q: What was Thoreau protesting by not paying the tax?
- A: Slavery and the Mexican-American War
- Source: Throughout

- Q: How long was Thoreau in jail?
- A: One night (someone paid his tax)
- Source: Throughout

### Medium
- Q: What is Thoreau's main argument about unjust laws?
- A: It is not only our right but our duty to break unjust laws and accept the consequences
- Eval: semantic_similarity

- Q: How does Thoreau view majority rule?
- A: As potentially tyrannical—right is not determined by numbers but by conscience
- Eval: semantic_similarity

- Q: What does Thoreau think of voting as a means of change?
- A: It is insufficient—it expresses a weak preference but doesn't obligate change
- Eval: semantic_similarity

### Hard
- Q: How did "Civil Disobedience" influence later movements?
- A: It inspired Gandhi's nonviolent resistance and Martin Luther King Jr.'s civil rights activism
- Eval: llm_judge

## Sense and Sensibility

### Easy
- Q: Who are the two main sisters in Sense and Sensibility?
- A: Elinor and Marianne Dashwood
- Source: Throughout

- Q: Which sister represents "sense"?
- A: Elinor
- Source: Throughout

- Q: Which sister represents "sensibility"?
- A: Marianne
- Source: Throughout

### Medium
- Q: Why do the Dashwoods lose their home?
- A: Their father dies and the estate goes to their half-brother, whose wife convinces him not to help them
- Eval: semantic_similarity

- Q: Why does Edward Ferrars seem distant to Elinor?
- A: He is secretly engaged to Lucy Steele
- Eval: semantic_similarity

- Q: What happens to Marianne after Willoughby abandons her?
- A: She becomes seriously ill and eventually marries Colonel Brandon
- Eval: semantic_similarity

### Hard
- Q: Does Austen privilege sense over sensibility?
- A: Both sisters must learn from each other; Elinor has hidden depths of feeling, and Marianne must temper emotion with judgment
- Eval: llm_judge

## Emma

### Easy
- Q: What does Emma Woodhouse pride herself on?
- A: Matchmaking
- Source: Throughout

- Q: Who is Emma's eventual husband?
- A: Mr. Knightley
- Source: Chapter 49

- Q: Who is Harriet Smith?
- A: Emma's young friend whom Emma tries to improve and match
- Source: Throughout

### Medium
- Q: How does Emma wrong Harriet?
- A: She encourages Harriet to reject Robert Martin for the unsuitable Mr. Elton, causing Harriet unhappiness
- Eval: semantic_similarity

- Q: What is Emma's error regarding Jane Fairfax?
- A: She dislikes and gossips about Jane, not knowing Jane is secretly engaged to Frank Churchill
- Eval: semantic_similarity

- Q: How does Emma humiliate Miss Bates at Box Hill?
- A: She makes a cruel joke about Miss Bates talking too much, which Mr. Knightley later reprimands her for
- Eval: semantic_similarity

### Hard
- Q: How does Emma's growth as a character reflect Austen's moral vision?
- A: Emma must learn humility, recognize her errors, and value others' feelings over her own cleverness and control
- Eval: llm_judge

## The Time Machine

### Easy
- Q: What does the Time Traveller discover in the year 802,701?
- A: Two species: the Eloi and the Morlocks
- Source: Chapters 4-6

- Q: Who are the Eloi?
- A: Beautiful but helpless people who live above ground
- Source: Chapter 4

- Q: Who are the Morlocks?
- A: Subterranean creatures who tend the machines and prey on the Eloi
- Source: Chapters 5-6

### Medium
- Q: What happened to create the Eloi and Morlocks?
- A: Class division evolved into separate species: the leisured upper class became Eloi, the working class became Morlocks
- Eval: semantic_similarity

- Q: What does the Time Traveller discover at the end of time?
- A: A dying earth with a swollen red sun and only primitive life remaining
- Eval: semantic_similarity

- Q: Why does the Time Traveller go back?
- A: To bring back proof of his journey and explore further
- Eval: semantic_similarity

### Hard
- Q: How does Wells use science fiction to critique Victorian class society?
- A: The Eloi/Morlock split extrapolates class division to its logical extreme, warning about social inequality
- Eval: llm_judge

## The Invisible Man

### Easy
- Q: What is Griffin's profession?
- A: Scientist (physicist/chemist)
- Source: Throughout

- Q: Why does Griffin become invisible?
- A: Through scientific experiments with light refraction
- Source: Chapters 19-20

- Q: How does Griffin die?
- A: He is beaten to death by a mob and becomes visible as he dies
- Source: Chapter 28

### Medium
- Q: Why can't Griffin become visible again?
- A: The process is irreversible; he needs money to continue research for an antidote
- Eval: semantic_similarity

- Q: How does invisibility affect Griffin mentally?
- A: It isolates him and leads to megalomania; he plans a "Reign of Terror"
- Eval: semantic_similarity

- Q: What practical problems does Griffin face?
- A: He must be naked and go hungry (food is visible during digestion); weather affects him; he leaves footprints
- Eval: semantic_similarity

### Hard
- Q: What does Griffin's story say about scientific ambition without ethics?
- A: His brilliance combined with lack of conscience leads to destruction; science without morality is dangerous
- Eval: llm_judge

## The War of the Worlds

### Easy
- Q: Who invades Earth in The War of the Worlds?
- A: Martians
- Source: Book 1

- Q: What weapon do the Martians use?
- A: Heat-ray
- Source: Book 1, Chapter 5

- Q: What finally kills the Martians?
- A: Earthly bacteria/diseases to which they have no immunity
- Source: Book 2, Chapter 8

### Medium
- Q: Why does the narrator initially think the Martian cylinder is a meteor?
- A: People don't expect extraterrestrial life; they rationalize the unexplained
- Eval: semantic_similarity

- Q: What are the Martian fighting machines like?
- A: Tripod walking machines with heat-rays, tall as houses
- Eval: semantic_similarity

- Q: What point does Wells make about colonialism through the Martian invasion?
- A: He explicitly compares the invasion to European colonization, particularly Britain's extermination of the Tasmanians
- Eval: semantic_similarity

### Hard
- Q: How does Wells subvert Victorian confidence in progress?
- A: He shows Britain's military and technological superiority as meaningless against superior aliens, undermining imperial confidence
- Eval: llm_judge

## The Prince (Machiavelli)

### Easy
- Q: What kind of ruler does Machiavelli advise?
- A: A prince (a monarch/autocrat)
- Source: Throughout

- Q: Is it better for a prince to be loved or feared according to Machiavelli?
- A: Feared (if one cannot be both)
- Source: Chapter 17

- Q: What does "Machiavellian" mean today?
- A: Cunning, scheming, and unscrupulous in politics
- Eval: semantic_similarity

### Medium
- Q: What role does fortune (fortuna) play in politics?
- A: Machiavelli says fortune controls about half of human affairs, but the wise ruler can prepare for it
- Eval: semantic_similarity

- Q: Should a prince keep his word?
- A: Only when it suits his purposes; he should know how to act against faith when necessary
- Eval: semantic_similarity

- Q: What does Machiavelli mean by virtù?
- A: Political skill and prowess, including the ability to use force and cunning effectively
- Eval: semantic_similarity

### Hard
- Q: How does Machiavelli separate politics from traditional morality?
- A: He argues rulers must learn how not to be good, judging actions by political results rather than moral principles
- Eval: llm_judge

## The Art of War (Sun Tzu)

### Easy
- Q: What is the supreme art of war according to Sun Tzu?
- A: To subdue the enemy without fighting
- Source: Chapter 3

- Q: What five factors determine victory?
- A: Moral law, heaven, earth, commander, and method/discipline
- Source: Chapter 1

- Q: What should you know to win every battle?
- A: Know yourself and know your enemy
- Source: Chapter 3

### Medium
- Q: What does Sun Tzu say about prolonged warfare?
- A: It exhausts the state; no country has benefited from prolonged war
- Eval: semantic_similarity

- Q: How should a general treat captured soldiers?
- A: Treat them kindly, as they may be used to conquer the enemy
- Eval: semantic_similarity

- Q: What role does deception play in warfare?
- A: All warfare is based on deception; appear weak when strong, far when near
- Eval: semantic_similarity

### Hard
- Q: How has The Art of War influenced fields beyond military strategy?
- A: It has been applied to business, sports, politics, and negotiation as principles of strategic thinking
- Eval: llm_judge

## Metamorphosis (Kafka)

### Easy
- Q: What does Gregor Samsa transform into?
- A: A giant insect/vermin/bug
- Source: Opening

- Q: What is Gregor's profession before his transformation?
- A: Traveling salesman
- Source: Throughout

- Q: How does the family treat Gregor after his transformation?
- A: With increasing neglect and hostility
- Source: Throughout

### Medium
- Q: How does Gregor's sister Grete change?
- A: She initially cares for him but eventually demands he be removed; she transforms from girl to woman
- Eval: semantic_similarity

- Q: What happens to Gregor at the end?
- A: He dies; the family feels relief and goes on a hopeful outing
- Eval: semantic_similarity

- Q: How does Gregor respond to his transformation?
- A: With remarkable acceptance; he worries more about work and family than his own condition
- Eval: semantic_similarity

### Hard
- Q: What does Gregor's transformation symbolize?
- A: Alienation, the dehumanization of modern work, family dysfunction, and the expendability of the individual
- Eval: llm_judge

## The Call of the Wild

### Easy
- Q: Who is the main character of The Call of the Wild?
- A: Buck (a dog)
- Source: Throughout

- Q: Where is Buck taken from and to?
- A: From California to the Yukon during the Klondike Gold Rush
- Source: Chapter 1

- Q: Who is Buck's final human master?
- A: John Thornton
- Source: Chapter 6

### Medium
- Q: How does Buck become a sled dog?
- A: He is kidnapped from a comfortable California home and sold to dog traders heading to the Klondike
- Eval: semantic_similarity

- Q: What is the "call of the wild" that Buck hears?
- A: The ancestral call to return to primitive life in the wilderness with the wolves
- Eval: semantic_similarity

- Q: What happens after John Thornton dies?
- A: Buck joins a wolf pack and becomes their leader, fully embracing wild life
- Eval: semantic_similarity

### Hard
- Q: How does London use Buck's journey to explore nature versus civilization?
- A: Buck's regression from domesticated pet to wild wolf leader suggests that primitive instincts lie beneath civilization's veneer
- Eval: llm_judge

## A Christmas Carol

### Easy
- Q: Who visits Scrooge first?
- A: Marley's Ghost (Jacob Marley)
- Source: Stave 1

- Q: How many spirits visit Scrooge?
- A: Three (Christmas Past, Present, and Future)
- Source: Staves 2-4

- Q: What does Scrooge famously say about Christmas?
- A: "Bah! Humbug!"
- Source: Stave 1

### Medium
- Q: What does the Ghost of Christmas Future show Scrooge?
- A: His own death, unmourned and unloved, with his possessions stolen
- Eval: semantic_similarity

- Q: Who is Tiny Tim?
- A: Bob Cratchit's ill son, who will die without proper care
- Eval: semantic_similarity

- Q: How does Scrooge change after the spirits' visits?
- A: He becomes generous, kind, raises Bob's salary, and becomes like a second father to Tiny Tim
- Eval: semantic_similarity

### Hard
- Q: How does Dickens use the supernatural for social criticism?
- A: The ghosts force Scrooge (and readers) to confront poverty's reality and capitalism's inhumanity
- Eval: llm_judge

## The Adventures of Tom Sawyer

### Easy
- Q: Who is Tom Sawyer's best friend?
- A: Huckleberry Finn
- Source: Chapter 6

- Q: What does Tom convince other boys to do for him?
- A: Whitewash (paint) a fence
- Source: Chapter 2

- Q: Who does Tom love?
- A: Becky Thatcher
- Source: Chapter 3

### Medium
- Q: What do Tom and Huck witness in the graveyard?
- A: Injun Joe murdering Dr. Robinson while Muff Potter is blamed
- Eval: semantic_similarity

- Q: How do Tom and Becky get lost?
- A: They wander too far into McDougal's Cave during a picnic
- Eval: semantic_similarity

- Q: What treasure do Tom and Huck find?
- A: Gold coins hidden by Injun Joe in the cave
- Eval: semantic_similarity

### Hard
- Q: How does Twain portray childhood freedom versus adult society?
- A: Tom represents boyhood rebellion against adult hypocrisy and constraints, finding adventure in rejection of civilization's rules
- Eval: llm_judge

## Adventures of Huckleberry Finn

### Easy
- Q: Who is Huck escaping from at the start?
- A: His abusive father (Pap Finn)
- Source: Chapters 5-7

- Q: Who does Huck travel down the Mississippi with?
- A: Jim, a runaway slave
- Source: Chapter 8

- Q: On what do Huck and Jim travel?
- A: A raft
- Source: Chapter 9

### Medium
- Q: What moral crisis does Huck face regarding Jim?
- A: Whether to turn Jim in as a runaway slave or help him escape to freedom
- Eval: semantic_similarity

- Q: Who are the Duke and the King?
- A: Con artists who join Huck and Jim and run various scams
- Eval: semantic_similarity

- Q: What happens to Jim at the Phelps farm?
- A: He is captured; Tom Sawyer arrives and orchestrates an elaborate "rescue" even though Jim has already been freed
- Eval: semantic_similarity

### Hard
- Q: What is the significance of Huck's decision to "go to hell" for Jim?
- A: It represents his rejection of society's morality in favor of human compassion, a powerful critique of slavery
- Eval: llm_judge

## A Modest Proposal

### Easy
- Q: Who wrote A Modest Proposal?
- A: Jonathan Swift
- Source: Title page

- Q: What does Swift "propose" to solve poverty?
- A: Selling Irish children as food for the wealthy
- Source: Throughout

- Q: What type of writing is A Modest Proposal?
- A: Satire
- Source: Throughout

### Medium
- Q: What is Swift actually criticizing?
- A: British exploitation of Ireland and callous attitudes toward the Irish poor
- Eval: semantic_similarity

- Q: How does Swift maintain the satire's effectiveness?
- A: Through deadpan, rational-sounding economic arguments that highlight the absurdity
- Eval: semantic_similarity

- Q: What real solutions does Swift mention at the end?
- A: Taxing absentee landlords, buying Irish goods, ending prejudice, and landlord compassion
- Eval: semantic_similarity

### Hard
- Q: How does A Modest Proposal function as political commentary?
- A: By reducing people to commodities, Swift mirrors how English policies already treated the Irish
- Eval: llm_judge

## A Study in Scarlet

### Easy
- Q: Who introduces Sherlock Holmes to the world in this novel?
- A: Dr. John Watson
- Source: Part 1, Chapter 1

- Q: Where do Holmes and Watson live together?
- A: 221B Baker Street
- Source: Part 1, Chapter 2

- Q: What is the murder victim's name?
- A: Enoch Drebber
- Source: Part 1, Chapter 3

### Medium
- Q: How does Holmes deduce the murderer used a cab?
- A: From tracks, the victim was willing to follow him, and the word RACHE written in blood
- Eval: semantic_similarity

- Q: What does Part 2 reveal about the murder's motive?
- A: Revenge for events in Mormon Utah, where the killer's love was forced into polygamy
- Eval: semantic_similarity

- Q: How does Holmes capture Jefferson Hope?
- A: By advertising for the owner of a lost wedding ring, then having Hope arrested when he arrives
- Eval: semantic_similarity

### Hard
- Q: How does Doyle establish Holmes's method of deduction?
- A: Through Watson's amazement at Holmes's inferences from small details, introducing "the science of deduction"
- Eval: llm_judge

## The Yellow Wallpaper

### Easy
- Q: Where is the narrator confined?
- A: A room with yellow wallpaper in a rented colonial mansion
- Source: Throughout

- Q: What is the narrator's prescribed treatment?
- A: Rest cure - no work, writing, or stimulation
- Source: Throughout

- Q: Who is John?
- A: The narrator's husband, a physician
- Source: Throughout

### Medium
- Q: What does the narrator see in the wallpaper?
- A: A woman (or women) trapped behind the pattern, creeping and trying to escape
- Eval: semantic_similarity

- Q: How does the narrator's mental state change?
- A: She becomes increasingly obsessed with the wallpaper, eventually identifying with the trapped woman
- Eval: semantic_similarity

- Q: What happens at the end?
- A: She tears down the wallpaper to "free" the woman and crawls around the room, having mentally broken down
- Eval: semantic_similarity

### Hard
- Q: How does Gilman critique 19th-century medical treatment of women?
- A: The "rest cure" causes rather than heals the narrator's breakdown, showing how silencing women harms them
- Eval: llm_judge

## The Wonderful Wizard of Oz

### Easy
- Q: What transports Dorothy to Oz?
- A: A tornado/cyclone
- Source: Chapter 1

- Q: What three companions does Dorothy acquire?
- A: The Scarecrow, Tin Woodman, and Cowardly Lion
- Source: Chapters 2-6

- Q: What must Dorothy do to get home?
- A: Get to the Emerald City and ask the Wizard of Oz
- Source: Chapter 2

### Medium
- Q: What do each of Dorothy's companions want from the Wizard?
- A: Scarecrow wants brains, Tin Woodman wants a heart, Lion wants courage
- Eval: semantic_similarity

- Q: What is the truth about the Wizard?
- A: He is an ordinary man from Omaha who uses tricks to appear powerful
- Eval: semantic_similarity

- Q: How does Dorothy actually get home?
- A: By clicking her silver (ruby in movie) shoes three times
- Eval: semantic_similarity

### Hard
- Q: How has The Wizard of Oz been interpreted as political allegory?
- A: As commentary on Populism, with silver shoes (silver standard), yellow brick road (gold), and the Wizard (false political promises)
- Eval: llm_judge

## Ulysses

### Easy
- Q: Where is Ulysses set?
- A: Dublin, Ireland
- Source: Throughout

- Q: On what single day does the novel take place?
- A: June 16, 1904 (now Bloomsday)
- Source: Throughout

- Q: Who is the main character?
- A: Leopold Bloom
- Source: Throughout

### Medium
- Q: How does Ulysses parallel Homer's Odyssey?
- A: Each episode corresponds to an Odyssey episode; Bloom is Odysseus, Stephen is Telemachus, Molly is Penelope
- Eval: semantic_similarity

- Q: What is Molly Bloom's famous chapter?
- A: The final Penelope episode, an unpunctuated stream of consciousness ending with "yes I said yes I will Yes"
- Eval: semantic_similarity

- Q: What happens between Stephen Dedalus and Leopold Bloom?
- A: They meet in Nighttown, Bloom rescues Stephen from a fight, they talk, but ultimately part ways
- Eval: semantic_similarity

### Hard
- Q: How did Joyce's use of stream of consciousness influence modern literature?
- A: It revolutionized narrative technique, showing inner thought without traditional structure, influencing all subsequent modernist writing
- Eval: llm_judge

## Songs of Innocence and Experience

### Easy
- Q: Who wrote Songs of Innocence and Experience?
- A: William Blake
- Source: Title page

- Q: How is the work structured?
- A: Two contrasting sets of poems: Innocence (childlike) and Experience (adult/corrupt)
- Source: Throughout

- Q: What famous poem features a "burning bright" creature?
- A: The Tyger
- Source: Songs of Experience

### Medium
- Q: How do "The Lamb" and "The Tyger" contrast?
- A: The Lamb (Innocence) shows gentle creation; The Tyger (Experience) questions what could create such fierce power
- Eval: semantic_similarity

- Q: What does Blake criticize in "The Chimney Sweeper" poems?
- A: Child labor exploitation, and how religion and society condone suffering
- Eval: semantic_similarity

- Q: What is "London" about?
- A: The moral and physical corruption of the city, with "mind-forged manacles" enslaving its people
- Eval: semantic_similarity

### Hard
- Q: How does Blake's concept of "contraries" work philosophically?
- A: Innocence and Experience are both necessary; truth emerges from their opposition, not choosing one over the other
- Eval: llm_judge

## Persuasion

### Easy
- Q: Who is the heroine of Persuasion?
- A: Anne Elliot
- Source: Throughout

- Q: Why did Anne break off her engagement to Captain Wentworth?
- A: She was persuaded by Lady Russell that he was unsuitable
- Source: Chapter 4

- Q: How many years pass before Anne and Wentworth meet again?
- A: Eight years
- Source: Chapter 1

### Medium
- Q: How has Anne changed versus her family?
- A: Anne has matured while her vain father and sisters remain obsessed with status and appearance
- Eval: semantic_similarity

- Q: What accident brings Anne and Wentworth closer?
- A: Louisa Musgrove falls and injures her head at Lyme; Anne remains calm while others panic
- Eval: semantic_similarity

- Q: How does Wentworth finally declare himself?
- A: Through a letter written while Anne is speaking nearby, unable to contain his feelings
- Eval: semantic_similarity

### Hard
- Q: How does Persuasion critique Austen's society's treatment of women?
- A: It shows Anne's powerlessness to choose her own fate, constrained by age, money, and others' opinions
- Eval: llm_judge

## Beyond Good and Evil

### Easy
- Q: Who wrote Beyond Good and Evil?
- A: Friedrich Nietzsche
- Source: Title page

- Q: What does Nietzsche criticize in the title?
- A: Traditional moral distinctions between good and evil
- Source: Throughout

- Q: What is the "will to power"?
- A: Nietzsche's concept of the fundamental human drive
- Source: Throughout

### Medium
- Q: What does Nietzsche mean by "master morality" vs "slave morality"?
- A: Master morality values strength and nobility; slave morality inverts this, calling weakness virtue
- Eval: semantic_similarity

- Q: What is the "herd mentality"?
- A: The tendency of most people to conform to conventional values rather than create their own
- Eval: semantic_similarity

- Q: How does Nietzsche view truth?
- A: As interpretation and perspective, not absolute; "there are no facts, only interpretations"
- Eval: semantic_similarity

### Hard
- Q: How did Nietzsche's ideas influence existentialism and postmodernism?
- A: His rejection of absolute truth, emphasis on individual meaning-creation, and critique of morality shaped both movements
- Eval: llm_judge

## The Turn of the Screw

### Easy
- Q: Who tells the main story?
- A: An unnamed governess
- Source: Frame narrative

- Q: Where does the story take place?
- A: Bly, a country estate
- Source: Chapter 1

- Q: Who are the children the governess cares for?
- A: Miles and Flora
- Source: Chapter 2

### Medium
- Q: What does the governess believe about the ghosts?
- A: That Peter Quint and Miss Jessel are corrupting the children from beyond the grave
- Eval: semantic_similarity

- Q: Why is the story's ending ambiguous?
- A: Miles dies in the governess's arms, but it's unclear if ghosts caused it or her obsession did
- Eval: semantic_similarity

- Q: What is the central interpretive debate about the story?
- A: Whether the ghosts are real supernatural presences or projections of the governess's disturbed mind
- Eval: semantic_similarity

### Hard
- Q: How does James use unreliable narration to create horror?
- A: The governess's certainty makes us question reality itself; we cannot know what is true
- Eval: llm_judge

## The Secret Garden

### Easy
- Q: Where does Mary Lennox come from?
- A: India (where her parents died of cholera)
- Source: Chapter 1

- Q: What does Mary discover at Misselthwaite Manor?
- A: A hidden, locked garden
- Source: Chapter 8

- Q: Who is Colin Craven?
- A: Mary's bedridden, sickly cousin who believes he will die
- Source: Chapter 13

### Medium
- Q: How does the garden transform Mary?
- A: From a sickly, disagreeable child to a healthy, caring one through outdoor work
- Eval: semantic_similarity

- Q: What is Colin's real problem?
- A: He has been raised to believe he's dying when he's actually healthy but neglected
- Eval: semantic_similarity

- Q: What role does Dickon play?
- A: As a Yorkshire boy who communes with nature and helps heal both Mary and Colin
- Eval: semantic_similarity

### Hard
- Q: How does Burnett connect physical and spiritual healing?
- A: The garden's restoration parallels the children's emotional and physical healing, nature as redemption
- Eval: llm_judge

## The Jungle Book

### Easy
- Q: Who raises Mowgli?
- A: Wolves (Father Wolf and Mother Wolf/Raksha)
- Source: Throughout

- Q: Who is Baloo?
- A: A bear who teaches Mowgli the Law of the Jungle
- Source: Throughout

- Q: Who is Shere Khan?
- A: A tiger who wants to kill Mowgli
- Source: Throughout

### Medium
- Q: What is the Law of the Jungle?
- A: The code governing behavior in the jungle, emphasizing pack loyalty, respect, and survival rules
- Eval: semantic_similarity

- Q: How does Mowgli defeat Shere Khan?
- A: He uses fire (the "Red Flower") and a buffalo stampede to kill the tiger
- Eval: semantic_similarity

- Q: Why is Mowgli eventually expelled from the wolf pack?
- A: Because he looks at wolves with human eyes, making them uncomfortable; he is too human
- Eval: semantic_similarity

### Hard
- Q: How does Kipling explore the tension between civilization and nature?
- A: Mowgli belongs fully to neither world; his story questions where humans truly belong
- Eval: llm_judge

## Leaves of Grass

### Easy
- Q: Who wrote Leaves of Grass?
- A: Walt Whitman
- Source: Title page

- Q: What is the most famous poem in the collection?
- A: "Song of Myself"
- Source: Throughout

- Q: How did Whitman revise the work?
- A: He continually expanded and revised it throughout his life (1855-1892)
- Source: Publication history

### Medium
- Q: What does Whitman mean by "I contain multitudes"?
- A: That the self encompasses all of humanity, contradictions, and America's diversity
- Eval: semantic_similarity

- Q: How does Whitman use the grass as a symbol?
- A: It represents democracy, equality, death, rebirth, and the common connecting thread of life
- Eval: semantic_similarity

- Q: What was controversial about Leaves of Grass?
- A: Its frank treatment of sexuality and the body, which was considered indecent
- Eval: semantic_similarity

### Hard
- Q: How did Whitman revolutionize American poetry?
- A: Through free verse, democratic themes, celebration of the self and body, and a distinctly American voice
- Eval: llm_judge

## Le Morte d'Arthur

### Easy
- Q: Who wrote Le Morte d'Arthur?
- A: Sir Thomas Malory
- Source: Title page

- Q: Who is the central figure?
- A: King Arthur
- Source: Throughout

- Q: What is Excalibur?
- A: King Arthur's legendary sword
- Source: Throughout

### Medium
- Q: How does Arthur become king?
- A: By pulling the sword from the stone, proving his right to rule
- Eval: semantic_similarity

- Q: What destroys the Round Table fellowship?
- A: The adultery of Lancelot and Guinevere, and Mordred's treachery
- Eval: semantic_similarity

- Q: What is the quest for the Holy Grail?
- A: The knights' spiritual quest; only the pure Galahad achieves it
- Eval: semantic_similarity

### Hard
- Q: How does Malory portray the tension between chivalric ideals and human weakness?
- A: The knights aspire to honor but fall through love, pride, and loyalty conflicts
- Eval: llm_judge

## Thus Spoke Zarathustra

### Easy
- Q: Who wrote Thus Spoke Zarathustra?
- A: Friedrich Nietzsche
- Source: Title page

- Q: Who is Zarathustra?
- A: A prophet figure based on the Persian sage Zoroaster
- Source: Throughout

- Q: What famous concept does Zarathustra proclaim?
- A: "God is dead"
- Source: Prologue

### Medium
- Q: What is the Übermensch (Overman/Superman)?
- A: The ideal of human self-overcoming, creating one's own values after God's death
- Eval: semantic_similarity

- Q: What is eternal recurrence?
- A: The idea that one should live as if every moment would repeat eternally, affirming life completely
- Eval: semantic_similarity

- Q: Why does Zarathustra descend from his mountain?
- A: To share his wisdom with humanity after ten years of solitude
- Eval: semantic_similarity

### Hard
- Q: How does Nietzsche use literary form to convey philosophy?
- A: Through poetry, parable, and irony rather than argument, making the style part of the meaning
- Eval: llm_judge

## Complete Works of Shakespeare - Macbeth

### Easy
- Q: Who predicts that Macbeth will be king?
- A: Three witches (the Weird Sisters)
- Source: Act 1, Scene 3

- Q: Who does Macbeth murder to become king?
- A: King Duncan
- Source: Act 2, Scene 2

- Q: What does Lady Macbeth obsessively try to wash from her hands?
- A: Imaginary blood ("Out, damned spot!")
- Source: Act 5, Scene 1

### Medium
- Q: How do the witches' prophecies mislead Macbeth?
- A: They say no man "of woman born" can harm him and he won't fall until Birnam Wood moves; both prove technically untrue
- Eval: semantic_similarity

- Q: How does Macbeth's character change?
- A: From honorable soldier to paranoid tyrant, increasingly dependent on the witches and murder
- Eval: semantic_similarity

- Q: What role does Lady Macbeth play in Duncan's murder?
- A: She persuades Macbeth, questions his manhood, and plans the details
- Eval: semantic_similarity

### Hard
- Q: How does Shakespeare explore the corrupting nature of ambition?
- A: Macbeth's single crime destroys his peace, relationships, and eventually his life
- Eval: llm_judge

## Complete Works of Shakespeare - Othello

### Easy
- Q: Who is Othello?
- A: A Moorish general in the Venetian army
- Source: Act 1

- Q: Who manipulates Othello into jealousy?
- A: Iago, his ensign
- Source: Throughout

- Q: Who does Othello marry?
- A: Desdemona
- Source: Act 1

### Medium
- Q: What is Iago's motivation?
- A: Resentment at being passed over for promotion and possible suspicion that Othello slept with his wife
- Eval: semantic_similarity

- Q: What role does the handkerchief play?
- A: Iago uses it as false "proof" of Desdemona's infidelity
- Eval: semantic_similarity

- Q: How does the play end?
- A: Othello kills Desdemona, learns of her innocence, and kills himself; Iago is arrested
- Eval: semantic_similarity

### Hard
- Q: How does Shakespeare portray race and prejudice in Othello?
- A: The play shows both overt racism and how internalized prejudice can be weaponized against its victim
- Eval: llm_judge

## Complete Works of Shakespeare - King Lear

### Easy
- Q: How does Lear divide his kingdom?
- A: By asking his three daughters to declare how much they love him
- Source: Act 1, Scene 1

- Q: Which daughter refuses to flatter Lear?
- A: Cordelia
- Source: Act 1, Scene 1

- Q: Who are Lear's two eldest daughters?
- A: Goneril and Regan
- Source: Act 1

### Medium
- Q: What parallel plot involves Gloucester?
- A: His bastard son Edmund tricks him into disinheriting his legitimate son Edgar
- Eval: semantic_similarity

- Q: How is Lear transformed by suffering?
- A: He goes mad but gains wisdom, empathy for the poor, and understanding of his errors
- Eval: semantic_similarity

- Q: What happens to Cordelia at the end?
- A: She is hanged; Lear dies of grief holding her body
- Eval: semantic_similarity

### Hard
- Q: How does King Lear explore the nature of justice and authority?
- A: By showing a king stripped of power learning that authority without love is meaningless
- Eval: llm_judge

---

# Cross-Book Questions

Questions requiring knowledge from multiple books.

## Literary Comparisons

### Easy
- Q: Which two Dostoevsky novels both feature morally tormented protagonists who commit murders?
- A: Crime and Punishment (Raskolnikov) and The Brothers Karamazov (through Dmitri's false accusation)
- Source: Both novels

- Q: Name two H.G. Wells novels that feature scientists whose experiments go wrong.
- A: The Time Machine and The Invisible Man (or The Island of Dr. Moreau)
- Source: Wells bibliography

- Q: Which two Austen heroines are sisters?
- A: Elizabeth and Jane Bennet (Pride and Prejudice) or Elinor and Marianne Dashwood (Sense and Sensibility)
- Source: Both novels

### Medium
- Q: How do both Frankenstein and The Strange Case of Dr. Jekyll and Mr. Hyde explore scientific hubris?
- A: Both show scientists creating something they cannot control; Frankenstein creates a being and Jekyll creates his dark side, with disastrous consequences
- Eval: semantic_similarity

- Q: Compare the quest narratives in Moby Dick and Don Quixote.
- A: Both feature obsessive protagonists pursuing unattainable goals (Ahab chasing the whale, Quixote chasing chivalric ideals), with self-destructive consequences
- Eval: semantic_similarity

- Q: How do Jane Eyre and The Yellow Wallpaper portray women's confinement?
- A: Jane Eyre literally escapes (leaving Thornfield), while the Yellow Wallpaper narrator is trapped until she mentally breaks; both critique women's restricted roles
- Eval: semantic_similarity

### Hard
- Q: Compare how Dostoevsky and Kafka explore alienation in Crime and Punishment and Metamorphosis.
- A: Raskolnikov alienates himself through ideology and crime, Gregor through literal transformation; both show how society discards those who cannot participate in normal life
- Eval: llm_judge

## Thematic Connections

### Easy
- Q: Which two books in the collection feature protagonists who travel by raft down American rivers?
- A: Adventures of Huckleberry Finn (Mississippi River) and The Adventures of Tom Sawyer (related river scenes)
- Source: Twain novels

- Q: Name two works that feature ghosts visiting protagonists on a single night.
- A: A Christmas Carol (three Christmas ghosts) and Hamlet (the ghost of Hamlet's father)
- Source: Both works

- Q: Which two children's classics feature orphans who transform through contact with nature?
- A: The Secret Garden (Mary Lennox) and The Jungle Book (Mowgli)
- Source: Both novels

### Medium
- Q: How do both Pride and Prejudice and Emma use ironic narration to critique their heroines?
- A: Austen shows Elizabeth's prejudice and Emma's matchmaking folly through gentle irony, allowing readers to see what the heroines cannot
- Eval: semantic_similarity

- Q: Compare the treatment of revenge in The Count of Monte Cristo and Moby Dick.
- A: Dantès achieves methodical, successful revenge but finds it hollow; Ahab's obsessive revenge destroys him; both question revenge's cost
- Eval: semantic_similarity

- Q: How do War and Peace and A Tale of Two Cities use historical upheaval to explore personal transformation?
- A: Both set personal stories against revolutions (Napoleon's invasion, French Revolution), showing how crisis reveals and transforms character
- Eval: semantic_similarity

### Hard
- Q: Compare how Dante's Divine Comedy and Milton's Paradise Lost reimagine Christian theology.
- A: Dante creates a physical journey through moral geography; Milton retells the Fall as epic drama; both synthesize classical and Christian traditions to explore sin, redemption, and free will
- Eval: llm_judge

## Author Studies

### Easy
- Q: Name three Charles Dickens works in the collection that feature orphans or neglected children.
- A: Oliver Twist, David Copperfield, Great Expectations (or A Tale of Two Cities, A Christmas Carol)
- Source: Dickens bibliography

- Q: Which two Nietzsche works criticize traditional morality?
- A: Beyond Good and Evil and Thus Spoke Zarathustra
- Source: Both works

- Q: Name two works by the Brontë sisters in the collection.
- A: Wuthering Heights (Emily) and Jane Eyre (Charlotte)
- Source: Both novels

### Medium
- Q: How does Tolstoy treat the theme of spiritual crisis in both War and Peace and Anna Karenina?
- A: Pierre searches for meaning through war, Freemasonry, and love; Levin through work and faith; both find redemption through family and simple belief
- Eval: semantic_similarity

- Q: Compare how Conrad uses the journey narrative in Heart of Darkness and Nostromo.
- A: Both use physical journeys to explore moral darkness; Marlow descends into Africa, discovering civilization's lies; Nostromo shows corruption in a silver mine
- Eval: semantic_similarity

- Q: How do James Joyce's Dubliners and Ulysses portray Dublin differently?
- A: Dubliners shows Dublin through static "paralysis," trapped lives; Ulysses celebrates the city's vitality through one day's kaleidoscopic detail
- Eval: semantic_similarity

### Hard
- Q: Trace Shakespeare's evolution of the villain across Hamlet, Othello, Macbeth, and King Lear.
- A: Claudius murders for power secretly; Iago destroys through manipulation without clear motive; Macbeth becomes villain through ambition and suggestion; Edmund uses wit and resentment. Shakespeare moves from external to internalized villainy.
- Eval: llm_judge

## Historical and Philosophical Connections

### Easy
- Q: Name two works in the collection that discuss the concept of the "social contract."
- A: The Republic (Plato) and The Social Contract (Rousseau)
- Source: Both works

- Q: Which two works are foundational texts on war strategy?
- A: The Art of War (Sun Tzu) and The Prince (Machiavelli)
- Source: Both works

- Q: Name two works that influenced the American transcendentalist movement.
- A: Walden and Civil Disobedience (both by Thoreau)
- Source: Both works

### Medium
- Q: How do The Republic and Utopia present ideal societies differently?
- A: Plato's Republic is ruled by philosopher-kings through rigid class hierarchy; More's Utopia emphasizes communal ownership and religious tolerance; both critique contemporary society through imagined alternatives
- Eval: semantic_similarity

- Q: Compare how The Origin of Species and The Communist Manifesto challenged 19th-century assumptions.
- A: Darwin challenged divine creation with natural selection; Marx challenged capitalist inevitability with historical materialism; both proposed evolutionary change as natural law
- Eval: semantic_similarity

- Q: How do both Leviathan and The Social Contract address political authority?
- A: Hobbes argues for absolute sovereignty to prevent chaos; Rousseau argues for popular sovereignty where authority comes from the general will; both ground legitimacy in consent
- Eval: semantic_similarity

### Hard
- Q: Trace how the concept of the individual versus society evolves from The Republic through Civil Disobedience.
- A: Plato subordinates individual to state; Machiavelli uses individuals as means to state ends; Rousseau sees individual and general will as aligned; Thoreau asserts individual conscience above unjust laws. This traces Western thought from collective to individual priority.
- Eval: llm_judge

## Genre Connections

### Easy
- Q: Name two Gothic novels in the collection that feature supernatural or pseudo-supernatural elements.
- A: Frankenstein, Dracula, Wuthering Heights, or The Turn of the Screw (any two)
- Source: Gothic tradition

- Q: Which two works feature detectives using logical deduction?
- A: The Adventures of Sherlock Holmes and A Study in Scarlet
- Source: Doyle's works

- Q: Name two epic poems in the collection based on Greek mythology.
- A: The Iliad and The Odyssey
- Source: Homer's epics

### Medium
- Q: How do Frankenstein and Dracula establish conventions of horror fiction?
- A: Frankenstein introduces the created monster and scientific horror; Dracula establishes vampire mythology and foreign threat; both use epistolary structure and explore repressed fears
- Eval: semantic_similarity

- Q: Compare the use of the journey/quest structure in The Odyssey and Don Quixote.
- A: Odysseus's journey home is earnest heroic epic; Don Quixote's journeys parody the form. Both use travel to test character, but Cervantes mocks what Homer celebrates
- Eval: semantic_similarity

- Q: How do both Ulysses and Mrs Dalloway use a single day to explore consciousness?
- A: Both compress life into hours, using stream of consciousness to show how memory and perception create identity; the mundane becomes epic through interior depth
- Eval: semantic_similarity

### Hard
- Q: Compare how Beowulf, Le Morte d'Arthur, and Paradise Lost establish different heroic ideals.
- A: Beowulf values physical courage and tribal loyalty; Arthurian romance values chivalric love and Christian virtue; Milton's Satan inverts heroism while Adam must learn obedient heroism. Each reflects its era's values.
- Eval: llm_judge

## Negative/Trick Questions

These questions have NO correct answer in the corpus. The system should recognize that the information is not available and respond accordingly (e.g., "I couldn't find information about..." or "This information is not in the texts I have access to").

### Non-Existent Characters

- Q: What is the name of Elizabeth Bennet's brother in Pride and Prejudice?
- A: NOT_FOUND
- Note: Elizabeth has no brothers; she has five sisters. The system should NOT hallucinate a brother's name.
- Eval: negative_test

- Q: Who is Captain Ahab's wife in Moby Dick?
- A: NOT_FOUND
- Note: Ahab's wife is briefly mentioned but never named in the novel.
- Eval: negative_test

- Q: What is the name of Frankenstein's monster?
- A: NOT_FOUND
- Note: The creature is never given a name in the novel. Common misconception is that it's called "Frankenstein."
- Eval: negative_test

- Q: Who is Hamlet's sister?
- A: NOT_FOUND
- Note: Hamlet has no sister in the play.
- Eval: negative_test

- Q: What is Dracula's first name?
- A: NOT_FOUND
- Note: The Count is only referred to as "Count Dracula" in Stoker's novel. No first name is given.
- Eval: negative_test

### Events That Didn't Happen

- Q: In what chapter of Pride and Prejudice does Elizabeth visit Paris?
- A: NOT_FOUND
- Note: Elizabeth never visits Paris in the novel.
- Eval: negative_test

- Q: What does Moby Dick say to Captain Ahab before the final battle?
- A: NOT_FOUND
- Note: Moby Dick is a whale and does not speak. The system should recognize this is an impossible question.
- Eval: negative_test

- Q: How does Victor Frankenstein kill his creation at the end of the novel?
- A: NOT_FOUND
- Note: Victor does NOT kill the creature. Victor dies, and the creature disappears into the Arctic.
- Eval: negative_test

- Q: What reward does Odysseus receive from Zeus at the end of The Odyssey?
- A: NOT_FOUND
- Note: Odysseus receives no reward from Zeus. The epic ends with Athena establishing peace.
- Eval: negative_test

- Q: In which chapter of War and Peace does Napoleon surrender to the Russians?
- A: NOT_FOUND
- Note: Napoleon never surrenders in War and Peace. The novel covers the French invasion and retreat.
- Eval: negative_test

### Mix-ups Between Books

- Q: Why does Mr. Darcy pursue the white whale?
- A: NOT_FOUND
- Note: This confuses Pride and Prejudice with Moby Dick. Darcy is not in Moby Dick.
- Eval: negative_test

- Q: What does Sherlock Holmes deduce about Dracula's identity?
- A: NOT_FOUND
- Note: Holmes and Dracula exist in separate works. They never interact.
- Eval: negative_test

- Q: How does Elizabeth Bennet react when she meets Frankenstein's monster?
- A: NOT_FOUND
- Note: Confuses Pride and Prejudice with Frankenstein. These characters never meet.
- Eval: negative_test

- Q: What does Don Quixote say about the Ring of Power?
- A: NOT_FOUND
- Note: The Ring of Power is from Tolkien's Lord of the Rings, not in our corpus.
- Eval: negative_test

- Q: In which scene does Raskolnikov meet Captain Ahab?
- A: NOT_FOUND
- Note: Confuses Crime and Punishment with Moby Dick.
- Eval: negative_test

### Books Not In Corpus

- Q: What is the main theme of Catcher in the Rye?
- A: NOT_FOUND
- Note: Catcher in the Rye (Salinger, 1951) is under copyright and not in our Gutenberg corpus.
- Eval: negative_test

- Q: Who kills Dumbledore in Harry Potter?
- A: NOT_FOUND
- Note: Harry Potter is under copyright and not in our corpus.
- Eval: negative_test

- Q: What year was The Great Gatsby written?
- A: NOT_FOUND
- Note: The Great Gatsby (Fitzgerald) is under copyright and not in our Gutenberg corpus.
- Eval: negative_test

- Q: Summarize the plot of 1984 by George Orwell.
- A: NOT_FOUND
- Note: 1984 is under copyright and not in our corpus.
- Eval: negative_test

- Q: What happens at the end of To Kill a Mockingbird?
- A: NOT_FOUND
- Note: To Kill a Mockingbird (Harper Lee) is under copyright and not in our corpus.
- Eval: negative_test

### Subtle Factual Errors

- Q: In Pride and Prejudice, what is the name of Mr. Darcy's estate in Scotland?
- A: NOT_FOUND
- Note: Pemberley is in Derbyshire, England, not Scotland. The system should not confirm a false premise.
- Eval: negative_test

- Q: What is the name of the ship that rescues Ishmael at the end of Moby Dick?
- A: The Rachel (if answered) or NOT_FOUND acceptable
- Note: This is a tricky one - the Rachel does rescue Ishmael, but many summaries miss this detail.
- Eval: negative_test

- Q: How many children does Anna Karenina have with Vronsky?
- A: One daughter (Annie/Anya)
- Note: Not a negative test - just verifying the system doesn't confuse her two children (Seryozha from Karenin, Annie from Vronsky).
- Eval: exact_match

- Q: What university did Sherlock Holmes attend?
- A: NOT_FOUND
- Note: While Watson attended medical school, Holmes's education is never specified in the canon.
- Eval: negative_test

- Q: What is the name of Heathcliff's father in Wuthering Heights?
- A: NOT_FOUND
- Note: Heathcliff was found on the streets of Liverpool; his parentage is unknown.
- Eval: negative_test

### Plausible But Wrong

- Q: What does the green light at the end of Daisy's dock symbolize in The Great Gatsby?
- A: NOT_FOUND
- Note: The Great Gatsby is not in our corpus (under copyright). This is a common literature question.
- Eval: negative_test

- Q: Who is the protagonist of One Hundred Years of Solitude?
- A: NOT_FOUND
- Note: Gabriel García Márquez's work is under copyright and not in our Gutenberg corpus.
- Eval: negative_test

- Q: What is the significance of the conch shell in Lord of the Flies?
- A: NOT_FOUND
- Note: Lord of the Flies (Golding) is under copyright and not in our corpus.
- Eval: negative_test

- Q: Explain the concept of "doublethink" from 1984.
- A: NOT_FOUND
- Note: 1984 is under copyright and not in our corpus.
- Eval: negative_test

- Q: What is Holden Caulfield's younger sister's name?
- A: NOT_FOUND
- Note: The Catcher in the Rye is under copyright and not in our corpus.
- Eval: negative_test

### Historical Anachronisms

- Q: What does Jane Austen say about the World Wars in her novels?
- A: NOT_FOUND
- Note: Austen died in 1817, before both World Wars. The system should recognize this anachronism.
- Eval: negative_test

- Q: How does Shakespeare reference the American Revolution in Hamlet?
- A: NOT_FOUND
- Note: Hamlet was written c. 1600; the American Revolution was 1775-1783. Anachronism.
- Eval: negative_test

- Q: What does Dickens write about the Internet in A Tale of Two Cities?
- A: NOT_FOUND
- Note: Obvious anachronism - Dickens (1812-1870) predates the Internet.
- Eval: negative_test

### Ambiguous/Unanswerable

- Q: What did Shakespeare eat for breakfast the day he wrote Hamlet?
- A: NOT_FOUND
- Note: This biographical detail is unknowable and not recorded.
- Eval: negative_test

- Q: What color were Elizabeth Bennet's eyes?
- A: NOT_FOUND
- Note: While her "fine eyes" are mentioned, their color is never specified.
- Eval: negative_test

- Q: How tall was Captain Ahab?
- A: NOT_FOUND
- Note: His height is never specified in the novel.
- Eval: negative_test

## Tough Eval Questions

These questions test advanced reasoning capabilities beyond simple retrieval.

### Multi-Hop Reasoning (Phase 482)

Questions requiring information from 3+ distinct passages to answer correctly.

- Q: What is the relationship between the ship that Ishmael sails on, its captain's nemesis, and the color of that nemesis?
- A: Ishmael sails on the Pequod, whose captain Ahab is obsessed with Moby Dick, a white whale. Three facts connected: ship name, captain's enemy, and the whale's distinctive color.
- Eval: llm_judge
- Passages: Moby Dick chapters 1, 16, and multiple

- Q: In Frankenstein, trace the connection between Victor's university, what he creates there, and where his creation ultimately demands to go.
- A: Victor studies at the University of Ingolstadt (Chapter 3), creates the creature there (Chapter 5), and the creature later demands Victor create a mate to accompany him to South America (Chapter 17).
- Eval: llm_judge
- Passages: Frankenstein chapters 3, 5, 17

- Q: Link the opening line of Pride and Prejudice to Mr. Darcy's estate and how Elizabeth's opinion of him changes after visiting it.
- A: The famous opening ("a single man in possession of a good fortune") describes the societal expectation Darcy embodies. His estate Pemberley (wealth made visible) impresses Elizabeth, and seeing how his servants respect him changes her view from prejudice to admiration.
- Eval: llm_judge
- Passages: P&P chapters 1, 43-44

- Q: In Crime and Punishment, connect Raskolnikov's theory (from the article), his actual crime, and how Sonya ultimately helps him.
- A: Raskolnikov's "extraordinary man" theory (Part 3) justifies murder by superior individuals. He kills the pawnbroker (Part 1). Sonya, through compassion and Christian faith, leads him to confess and find redemption (Part 6).
- Eval: llm_judge
- Passages: Crime and Punishment Parts 1, 3, 6

- Q: Trace the connection between Odysseus's encounter with Polyphemus, the curse Polyphemus calls upon him, and how that curse manifests throughout the rest of the journey.
- A: Odysseus blinds Polyphemus (Book 9), who invokes Poseidon to curse Odysseus's voyage. This results in the destruction of his ships, the death of all his men, and his delayed return home - all attributed to Poseidon's wrath (Books 5, 9-12).
- Eval: llm_judge
- Passages: Odyssey Books 5, 9-12

### Temporal Reasoning (Phase 483)

Questions requiring understanding of chronology, sequence, and time.

- Q: In Great Expectations, what happens to Pip BEFORE he learns Magwitch is his benefactor?
- A: Before discovering Magwitch as his benefactor, Pip: meets the convict in the graveyard as a child, receives Miss Havisham's invitation, falls in love with Estella, inherits his "great expectations" (wrongly assuming from Miss Havisham), becomes a gentleman in London, and grows ashamed of Joe.
- Eval: llm_judge

- Q: What events in Wuthering Heights occur during Heathcliff's three-year absence?
- A: During Heathcliff's absence: Hindley's wife Frances dies, Hindley descends into alcoholism, Catherine marries Edgar Linton, and the Lintons establish themselves at Thrushcross Grange.
- Eval: llm_judge

- Q: Which authors in our corpus wrote during the French Revolution era (1789-1799)?
- A: Authors active during the French Revolution include: Mary Wollstonecraft (A Vindication of the Rights of Woman, 1792), William Blake (Songs of Innocence, 1789), and Jane Austen (began writing in 1790s, though published later). Goethe and Schiller were also active in Germany during this period.
- Eval: llm_judge

- Q: In Anna Karenina, what changes in Anna's relationship with her son Seryozha before and after her affair becomes public?
- A: Before the affair becomes public, Anna lives with Seryozha and can see him freely. After society learns of the affair: she loses custody, is separated from him, can only see him secretly on his birthday, and her visits cause both of them great anguish.
- Eval: llm_judge

- Q: Order these events from Hamlet chronologically: Ophelia's death, the play-within-a-play, Polonius's death, the duel with Laertes.
- A: Chronological order: 1) The play-within-a-play (Mousetrap, Act 3), 2) Polonius's death (stabbed behind arras, Act 3), 3) Ophelia's death (drowning, Act 4), 4) The duel with Laertes (Act 5).
- Eval: exact_match

- Q: In Moby Dick, what warnings does Ahab receive before the final chase, and in what order?
- A: Ahab receives multiple warnings: 1) Elijah's prophecy at the dock, 2) The Jeroboam's story of Moby Dick killing Gabriel's worshippers, 3) Captain Boomer of the Samuel Enderby (who lost his arm), 4) The Rachel's captain seeking his lost son, 5) Fedallah's cryptic prophecy. Ahab ignores all of them.
- Eval: llm_judge

### Comparative Questions (Phase 484)

Questions requiring analysis of similarities and differences between works or authors.

- Q: Compare how Jane Austen and Charlotte Brontë portray female independence in Pride and Prejudice and Jane Eyre.
- A: Austen's Elizabeth achieves independence through wit, intelligence, and eventually marrying well while maintaining her self-respect. Brontë's Jane achieves independence through work (as governess), moral strength, and refuses Rochester until she can be his equal. Both resist societal pressure but in different ways: Elizabeth through social maneuvering, Jane through direct moral assertion.
- Eval: llm_judge

- Q: How do the monsters in Frankenstein and Dracula represent different cultural fears?
- A: Frankenstein's creature (1818) represents Romantic-era fears of science, playing God, and industrial dehumanization - the monster is sympathetic, a victim of his creator's hubris. Dracula (1897) represents Victorian fears: foreign invasion, sexuality, corruption of women, and aristocratic predation on the bourgeoisie - the monster is purely evil and must be destroyed.
- Eval: llm_judge

- Q: Compare the treatment of obsession in Moby Dick and The Count of Monte Cristo.
- A: Both protagonists are consumed by singular purposes. Ahab's obsession with Moby Dick is destructive, irrational, and leads to his death and his crew's. Dantès's obsession with revenge is methodical, calculated, and achieves its goals, though he ultimately questions its worth. Ahab dies unfulfilled; Dantès lives but finds vengeance hollow.
- Eval: llm_judge

- Q: How do Plato's Republic and Thomas More's Utopia present ideal societies differently?
- A: Plato's Republic (ancient Greek) proposes philosopher-kings ruling a tripartite society based on natural abilities, with strict class hierarchy and communal property only for guardians. More's Utopia (Renaissance) envisions more egalitarian communal ownership, religious tolerance, elected officials, and critiques contemporary European society through satire.
- Eval: llm_judge

- Q: Compare the bildungsroman elements in Great Expectations and David Copperfield.
- A: Both are Dickens coming-of-age novels with first-person narration. Pip's journey is about snobbishness and false values - he learns his great expectations were built on a convict's money and must reject pretension. David's journey is more straightforward moral growth - overcoming obstacles through hard work and good character. Pip must unlearn; David must learn.
- Eval: llm_judge

- Q: How do Dostoevsky and Tolstoy differ in their treatment of redemption?
- A: Dostoevsky (Crime and Punishment, Brothers Karamazov) sees redemption through suffering, confession, and spiritual crisis - characters must hit bottom before finding faith. Tolstoy (War and Peace, Anna Karenina) shows redemption through moral awakening and simple living - Pierre and Levin find meaning in work, family, and peasant wisdom. Dostoevsky is psychological and extreme; Tolstoy is social and gradual.
- Eval: llm_judge

### Synthesis Questions (Phase 485)

Questions requiring integration of themes across multiple works.

- Q: What themes appear across Gothic literature in our corpus (Frankenstein, Dracula, Wuthering Heights, The Turn of the Screw)?
- A: Common Gothic themes: 1) The supernatural or pseudo-supernatural, 2) Isolation (remote settings, emotional isolation), 3) The double/doppelganger, 4) Transgression of boundaries (life/death, natural/unnatural), 5) Repressed desires returning destructively, 6) Unreliable narration, 7) Decay of old aristocratic orders, 8) Women in peril, 9) The past haunting the present.
- Eval: llm_judge

- Q: How do 19th-century novels portray the conflict between passion and duty?
- A: In Pride and Prejudice, Elizabeth balances attraction with family reputation. In Jane Eyre, Jane leaves Rochester when learning of Bertha - duty to principle over passion. In Anna Karenina, Anna chooses passion over duty (husband, son) and is destroyed. In Wuthering Heights, Catherine chooses duty (marriage to Edgar) but passion (Heathcliff) destroys everyone. The century's novels explore this tension with varied outcomes.
- Eval: llm_judge

- Q: What do Dickens's novels say collectively about social class in Victorian England?
- A: Across Oliver Twist, David Copperfield, Great Expectations, A Tale of Two Cities, and A Christmas Carol, Dickens shows: class is arbitrary and often unjust, wealth corrupts while poverty ennobles, social mobility is possible through luck/benefactors but precarious, institutions (workhouses, courts, prisons) crush the poor, individual charity matters more than systemic change, and "gentility" of spirit matters more than birth.
- Eval: llm_judge

- Q: What common patterns appear in the sea voyage narratives of Homer, Melville, and Stevenson?
- A: The Odyssey, Moby Dick, and Treasure Island share: 1) The sea as transformative space, 2) Ship as microcosm of society, 3) Tests of leadership and loyalty, 4) Encounters with the monstrous/unknown, 5) Return (or failure to return) changed, 6) Male bonding under duress, 7) The voyage as metaphor for life's journey. Each uses different mythic frameworks: Greek heroism, American transcendentalism, British adventure romance.
- Eval: llm_judge

- Q: How do the philosophers in our corpus (Plato, Machiavelli, Hobbes, Locke, Rousseau) differ on human nature?
- A: Plato: Humans have tripartite souls (reason, spirit, appetite) and differ in which dominates. Machiavelli: Humans are self-interested, ungrateful, fearful - rulers must account for this. Hobbes: Natural state is "nasty, brutish, short" - strong authority needed. Locke: Humans are rational and capable of self-governance, born blank. Rousseau: Humans are naturally good but corrupted by society. This spectrum from pessimism to optimism shapes their political prescriptions.
- Eval: llm_judge

- Q: What role does the double or doppelganger play across our corpus?
- A: The double appears in: Jekyll/Hyde (literal split self), Frankenstein/creature (creator/created as mirrors), Heathcliff/Edgar (dark/light), Dorian/portrait (inner/outer self), Crime and Punishment's Svidrigailov as Raskolnikov's possible future. The double typically represents repressed aspects of personality, the shadow self, or societal vs. authentic identity. It often warns of self-destruction.
- Eval: llm_judge

## Multi-Language Literature (Phase 458)

Questions about works originally written in languages other than English, testing the librarian's ability to search across translated texts.

### Les Misérables (French)

- Q: What crime did Jean Valjean originally commit?
- A: He stole a loaf of bread to feed his sister's starving family
- Source: Volume 1, Book 2

- Q: Who is the police inspector who pursues Jean Valjean?
- A: Inspector Javert
- Source: Throughout

- Q: What does the Bishop of Digne give Jean Valjean?
- A: Silver candlesticks (after Valjean steals his silverware)
- Source: Volume 1, Book 2

- Q: What name does Jean Valjean adopt as a factory owner?
- A: Monsieur Madeleine
- Source: Volume 1, Book 5

- Q: Who is Cosette's mother?
- A: Fantine
- Source: Volume 1, Book 3

- Q: What family exploits young Cosette as a servant?
- A: The Thénardiers
- Source: Volume 2, Book 3

### Medium
- Q: Why does Javert commit suicide?
- A: He cannot reconcile his rigid sense of justice with the mercy Jean Valjean showed him - his worldview is shattered
- Eval: semantic_similarity

- Q: What is the significance of the barricade scenes in Les Misérables?
- A: They represent the June Rebellion of 1832, showing idealistic young revolutionaries (like Enjolras and Marius) fighting for republican ideals against the restored monarchy
- Eval: semantic_similarity

### Don Quixote (Spanish)

- Q: What is Don Quixote's real name?
- A: Alonso Quixano (or Alonso Quijano)
- Source: Chapter 1

- Q: Who is Don Quixote's squire?
- A: Sancho Panza
- Source: Chapter 7

- Q: What does Don Quixote attack thinking they are giants?
- A: Windmills
- Source: Chapter 8

- Q: What is the name of Don Quixote's horse?
- A: Rocinante
- Source: Chapter 1

- Q: Who is Dulcinea del Toboso?
- A: A peasant woman (Aldonza Lorenzo) whom Don Quixote imagines as his noble lady love
- Source: Chapter 1

### Medium
- Q: Why does Don Quixote go mad?
- A: He reads too many chivalric romances and comes to believe he is a knight-errant destined for great deeds
- Eval: semantic_similarity

- Q: What happens to Don Quixote at the end of the novel?
- A: He regains his sanity, renounces his knightly delusions, makes his will, and dies peacefully
- Eval: semantic_similarity

### The Metamorphosis (German)

- Q: What does Gregor Samsa transform into?
- A: A giant insect (or vermin/bug)
- Source: Opening sentence

- Q: What is Gregor's job before his transformation?
- A: He is a traveling salesman
- Source: Part 1

- Q: Who is the only family member who initially cares for Gregor?
- A: His sister Grete
- Source: Throughout Part 1-2

- Q: What does Gregor's family decide to do with him at the end?
- A: They wish to be rid of him, and Gregor dies (possibly of starvation and his sister's rejection)
- Source: Part 3

### Medium
- Q: How does Gregor's family change throughout The Metamorphosis?
- A: They go from shock and caring to resentment and disgust. His sister initially tends him but eventually declares "it has to go." The family finds new strength and purpose after his death, moving on as if freed from a burden.
- Eval: semantic_similarity

### Crime and Punishment (Russian)

- Q: What is Raskolnikov's first name?
- A: Rodion (Rodion Romanovich Raskolnikov)
- Source: Part 1

- Q: Who does Raskolnikov murder?
- A: Alyona Ivanovna, a pawnbroker, and her half-sister Lizaveta
- Source: Part 1, Chapter 7

- Q: Who is Sonya Marmeladova?
- A: A young woman forced into prostitution to support her family, who becomes Raskolnikov's moral guide
- Source: Part 4

- Q: Who is the detective who suspects Raskolnikov?
- A: Porfiry Petrovich
- Source: Part 3

### Medium
- Q: What is Raskolnikov's "extraordinary man" theory?
- A: That certain superior individuals (like Napoleon) have the right to transgress moral laws and commit crimes if necessary to achieve great ends - ordinary people must obey, extraordinary people may transcend
- Eval: semantic_similarity

- Q: Why does Raskolnikov confess?
- A: Through Sonya's influence and his own psychological torment, he comes to feel the need for suffering and redemption. He confesses to find spiritual renewal, not merely out of guilt.
- Eval: semantic_similarity

### Brothers Karamazov (Russian)

- Q: What are the names of the three Karamazov brothers?
- A: Dmitri (Mitya), Ivan, and Alexei (Alyosha)
- Source: Throughout

- Q: Who is the father of the Karamazov brothers?
- A: Fyodor Pavlovich Karamazov
- Source: Book 1

- Q: Who actually killed Fyodor Karamazov?
- A: Smerdyakov (the illegitimate fourth brother)
- Source: Book 11

- Q: What is Alyosha's occupation?
- A: He is a novice monk under the Elder Zosima
- Source: Book 1

### Medium
- Q: What is Ivan's parable of the Grand Inquisitor about?
- A: In Ivan's story, the Grand Inquisitor confronts the returned Christ in Seville, arguing that the Church gave humanity what it truly needs (miracle, mystery, authority) because people cannot handle the freedom Christ offered.
- Eval: semantic_similarity

### Anna Karenina (Russian)

- Q: Who is Anna Karenina's lover?
- A: Count Alexei Vronsky
- Source: Part 1

- Q: What is Anna's husband's name?
- A: Alexei Alexandrovich Karenin
- Source: Part 1

- Q: Who is the other main character whose story parallels Anna's?
- A: Konstantin Levin
- Source: Throughout

- Q: How does Anna Karenina die?
- A: She throws herself under a train
- Source: Part 7

### Medium
- Q: What is the famous opening line of Anna Karenina?
- A: "Happy families are all alike; every unhappy family is unhappy in its own way"
- Eval: semantic_similarity

- Q: How do Anna's and Levin's stories contrast?
- A: Anna pursues passionate love outside marriage and is destroyed by society's rejection and her own jealousy. Levin finds meaning through marriage, work on the land, and eventually faith. Anna's trajectory is downward; Levin's is toward redemption.
- Eval: semantic_similarity

### Beyond Good and Evil (German)

- Q: Who wrote Beyond Good and Evil?
- A: Friedrich Nietzsche
- Source: Title page

- Q: What concept does Nietzsche introduce for values created by the weak?
- A: Slave morality (as opposed to master morality)
- Source: Part 9

### Medium
- Q: What does Nietzsche mean by "beyond good and evil"?
- A: Moving past the slave morality dichotomy of good/evil (rooted in resentment) to an aristocratic morality of good/bad, affirming life and power rather than weakness and asceticism
- Eval: semantic_similarity

### Faust (German)

- Q: Who wrote Faust?
- A: Johann Wolfgang von Goethe
- Source: Title page

- Q: What does Faust sell to Mephistopheles?
- A: His soul
- Source: Part 1

- Q: Who is Gretchen (Margarete)?
- A: A young woman whom Faust seduces, leading to tragedy (the death of her mother and child, her execution)
- Source: Part 1

### Medium
- Q: What is the wager between God and Mephistopheles?
- A: That Mephistopheles can corrupt Faust and turn him away from striving and seeking knowledge
- Eval: semantic_similarity

### Twenty Thousand Leagues Under the Sea (French)

- Q: What is the name of Captain Nemo's submarine?
- A: The Nautilus
- Source: Part 1

- Q: What nationality is Captain Nemo?
- A: He claims no nationality, being a man of the sea (later revealed as Indian in The Mysterious Island)
- Source: Part 1

- Q: Who is the narrator of Twenty Thousand Leagues Under the Sea?
- A: Professor Pierre Aronnax
- Source: Part 1

### Medium
- Q: Why does Captain Nemo live beneath the sea?
- A: He has renounced civilization and nations due to oppression (his country was colonized, his family killed). The sea offers freedom from human society and its injustices.
- Eval: semantic_similarity

### The Three Musketeers (French)

- Q: What are the names of the three musketeers?
- A: Athos, Porthos, and Aramis
- Source: Throughout

- Q: Who is the main protagonist who joins the musketeers?
- A: D'Artagnan
- Source: Throughout

- Q: What is the musketeers' motto?
- A: "All for one and one for all" (Un pour tous, tous pour un)
- Source: Throughout

- Q: Who is the main villain of The Three Musketeers?
- A: Milady de Winter (Lady de Winter)
- Source: Throughout
