# CardBrick CLI Developer Guide
## Complete Reference for Frontend Integration

### Table of Contents
1. [Quick Start](#quick-start)
2. [Installation & Setup](#installation--setup)
3. [Core Concepts](#core-concepts)
4. [Data Models](#data-models)
5. [Complete Workflow Examples](#complete-workflow-examples)
6. [Command Reference](#command-reference)
7. [Error Handling](#error-handling)
8. [Integration Examples](#integration-examples)
9. [Testing Guide](#testing-guide)
10. [Best Practices](#best-practices)
11. [Troubleshooting](#troubleshooting)

---

## Quick Start

### What is CardBrick CLI?
CardBrick CLI is a command-line interface that provides complete access to all CardBrick spaced repetition data and functionality. It enables you to build custom frontends (web apps, mobile apps, desktop apps) that use the same learning algorithms and data as the main CardBrick application.

### Why Use the CLI?
- **A/B Testing**: Build different UIs that share the same learning data
- **Custom Frontends**: Create specialized interfaces for different devices or use cases
- **Data Integration**: Connect CardBrick to other learning systems
- **Automation**: Build scripts for bulk operations or analytics

### 30-Second Test
```bash
# Build the CLI
cargo build --release --bin cardbrick-cli

# List available decks
./target/release/cardbrick-cli deck list

# You should see JSON output with your cached decks
```

---

## Installation & Setup

### Prerequisites
1. **Rust Environment**: You need Rust and Cargo installed
2. **CardBrick Data**: You need cached deck data in the proper format
3. **Platform**: Works on Linux, macOS, and Windows

### Step 1: Build the CLI
```bash
# Navigate to the CardBrick project directory
cd /path/to/CardBrick

# Build the CLI binary
cargo build --release --bin cardbrick-cli

# The binary will be at: ./target/release/cardbrick-cli
```

### Step 2: Verify Installation
```bash
# Test the CLI is working
./target/release/cardbrick-cli --help

# You should see the help output
```

### Step 3: Check Data Availability
```bash
# List available decks
./target/release/cardbrick-cli deck list

# If you see an empty list, you need to cache some decks first
# Refer to the main CardBrick documentation for deck caching
```

### Platform-Specific Notes

#### Linux/macOS
```bash
# Make the binary executable if needed
chmod +x ./target/release/cardbrick-cli

# Optionally, copy to a location in your PATH
sudo cp ./target/release/cardbrick-cli /usr/local/bin/
```

#### Windows
```cmd
# Use the .exe extension
./target/release/cardbrick-cli.exe --help
```

---

## Core Concepts

### Understanding CardBrick's Architecture

#### 1. Decks and Cards
- **Deck**: A collection of flashcards (like "Japanese Vocabulary")
- **Card**: A single flashcard with a question and answer
- **Note**: The content data for a card (fields like front, back, hints)

#### 2. Spaced Repetition System (SRS)
- **SM-2 Algorithm**: Determines when to show cards next
- **Interval**: Days until the card is due for review
- **Ease Factor**: How easy/hard the card is (affects future intervals)
- **Repetitions**: How many times you've seen the card
- **Lapses**: How many times you've failed the card

#### 3. Ratings
When you review a card, you rate your performance:
- **Again** (1): Failed, show again soon
- **Hard** (2): Difficult, shorter interval
- **Good** (3): Normal, standard interval
- **Easy** (4): Easy, longer interval

#### 4. Progress Tracking
- **Daily Progress**: Cards studied and ratings for today
- **Difficult Cards**: Cards you marked as Hard or Again today
- **Points System**: Scoring based on performance and difficulty
- **Streaks**: Consecutive days of study

#### 5. Sessions
- **Session**: A study period with a specific deck
- **Session State**: Tracks progress within a session
- **Session Management**: Start, study, and end sessions

---

## Data Models

### Understanding the JSON Data Structures

#### Card Object
```json
{
  "id": 1286040963115,           // Unique card identifier
  "note_id": 1286040963115,      // Associated note ID
  "due": 3412,                   // Due date (days since epoch)
  "interval": 6,                 // Current interval in days
  "ease_factor": 2500,           // Ease factor (* 1000)
  "lapses": 2                    // Number of times failed
}
```

#### Note Object
```json
{
  "id": 1286040963115,           // Note identifier
  "fields": [                    // Array of field values
    "現像",                       // Field 1: Japanese word
    "げんぞう",                   // Field 2: Reading
    "developing (film)"          // Field 3: English meaning
  ]
}
```

#### Session Object
```json
{
  "session_id": "uuid-string",   // Unique session identifier
  "deck_id": "deck-hash",        // Which deck is being studied
  "created_at": "2025-09-23T...", // When session started
  "cards_studied": 5,            // Cards reviewed in this session
  "current_card": { ... }        // Current card object (or null)
}
```

#### SRS State Object
```json
{
  "interval": 6,                 // Days until next review
  "ease_factor": 2.5,           // Difficulty multiplier
  "repetitions": 3,             // Times successfully reviewed
  "lapses": 1                   // Times failed
}
```

#### Progress Object
```json
{
  "date": "2025-09-23",         // Date in YYYY-MM-DD format
  "ratings": [                  // Array of [card_id, rating] pairs
    [1286040963115, "good"],
    [1286040963116, "hard"]
  ],
  "total_points": 45,           // Points earned today
  "cards_studied": 2            // Number of cards reviewed
}
```

---

## Complete Workflow Examples

### Example 1: Basic Study Session

This example shows a complete study session from start to finish.

#### Step 1: Find a Deck to Study
```bash
# List all available decks
./target/release/cardbrick-cli deck list
```

**Expected Output:**
```json
{
  "decks": [
    {
      "id": "7766cd891981a68c147e026fdef1520548f9fed81618c0b114749bd7eeacb0d6",
      "name": "JLPT N1 Vocab",
      "path": "./test_cache/7766cd891981a68c147e026fdef1520548f9fed81618c0b114749bd7eeacb0d6"
    }
  ]
}
```

#### Step 2: Get Deck Information
```bash
# Get detailed information about the deck
DECK_ID="7766cd891981a68c147e026fdef1520548f9fed81618c0b114749bd7eeacb0d6"
./target/release/cardbrick-cli deck info $DECK_ID
```

**Expected Output:**
```json
{
  "id": "7766cd891981a68c147e026fdef1520548f9fed81618c0b114749bd7eeacb0d6",
  "name": "JLPT N1 Vocab",
  "path": "./test_cache/7766cd891981a68c147e026fdef1520548f9fed81618c0b114749bd7eeacb0d6",
  "manifest": {
    "card_count": 2500,
    "deck_name": "JLPT N1 Vocab",
    "notes_count": 2500
  }
}
```

#### Step 3: Start a Study Session
```bash
# Start a new study session
./target/release/cardbrick-cli session start $DECK_ID
```

**Expected Output:**
```json
{
  "session_id": "abc123-def456-ghi789",
  "deck_id": "7766cd891981a68c147e026fdef1520548f9fed81618c0b114749bd7eeacb0d6",
  "created_at": "2025-09-23T10:30:00Z",
  "status": "started"
}
```

**Important:** Save the `session_id` - you'll need it for all subsequent operations!

#### Step 4: Get the First Card
```bash
# Use the session_id from the previous step
SESSION_ID="abc123-def456-ghi789"
./target/release/cardbrick-cli session next $SESSION_ID
```

**Expected Output:**
```json
{
  "session_id": "abc123-def456-ghi789",
  "card": {
    "id": 1286040963115,
    "note_id": 1286040963115,
    "due": 3412,
    "interval": 0,
    "ease_factor": 2500,
    "lapses": 0
  },
  "card_status": "new",
  "has_more": true
}
```

**Card Status Meanings:**
- `"new"`: Card hasn't been studied before
- `"due"`: Card is due for review now
- `"future"`: Card isn't due yet, but no other cards available

#### Step 5: Get the Card Content
```bash
# Get the note content for the card
CARD_ID=1286040963115
./target/release/cardbrick-cli deck note $DECK_ID $CARD_ID
```

**Expected Output:**
```json
{
  "deck_id": "7766cd891981a68c147e026fdef1520548f9fed81618c0b114749bd7eeacb0d6",
  "note": {
    "id": 1286040963115,
    "fields": [
      "現像",
      "げんぞう",
      "developing (film)"
    ]
  }
}
```

**Field Interpretation (depends on your deck):**
- Field 0: Usually the "front" of the card (question)
- Field 1: Often pronunciation or hint
- Field 2: Usually the "back" of the card (answer)

#### Step 6: Rate the Card
```bash
# User has reviewed the card and chosen their rating
# Ratings: "again", "hard", "good", "easy"
./target/release/cardbrick-cli session rate $SESSION_ID $CARD_ID "good"
```

**Expected Output:**
```json
{
  "session_id": "abc123-def456-ghi789",
  "card_id": 1286040963115,
  "rating": "good",
  "cards_studied": 1,
  "status": "rated",
  "timestamp": 1695509972
}
```

#### Step 7: Continue or End Session
```bash
# Get the next card (repeat steps 4-6)
./target/release/cardbrick-cli session next $SESSION_ID

# OR check session status
./target/release/cardbrick-cli session status $SESSION_ID

# OR end the session
./target/release/cardbrick-cli session end $SESSION_ID
```

### Example 2: Building a Frontend Loop

Here's how you might implement this in a frontend application:

#### JavaScript/Node.js Example
```javascript
const { exec } = require('child_process');
const util = require('util');
const execAsync = util.promisify(exec);

class CardBrickCLI {
  constructor(cliPath = './target/release/cardbrick-cli') {
    this.cliPath = cliPath;
  }

  async runCommand(args) {
    try {
      const { stdout, stderr } = await execAsync(`${this.cliPath} ${args}`);
      if (stderr) {
        throw new Error(stderr);
      }
      return JSON.parse(stdout);
    } catch (error) {
      console.error('CLI Error:', error.message);
      throw error;
    }
  }

  async listDecks() {
    return await this.runCommand('deck list');
  }

  async startSession(deckId) {
    return await this.runCommand(`session start ${deckId}`);
  }

  async getNextCard(sessionId) {
    return await this.runCommand(`session next ${sessionId}`);
  }

  async getNote(deckId, noteId) {
    return await this.runCommand(`deck note ${deckId} ${noteId}`);
  }

  async rateCard(sessionId, cardId, rating) {
    return await this.runCommand(`session rate ${sessionId} ${cardId} ${rating}`);
  }

  async endSession(sessionId) {
    return await this.runCommand(`session end ${sessionId}`);
  }
}

// Example usage
async function studySession() {
  const cli = new CardBrickCLI();

  try {
    // Get available decks
    const decks = await cli.listDecks();
    const deck = decks.decks[0]; // Use first deck

    // Start session
    const session = await cli.startSession(deck.id);
    console.log(`Started session: ${session.session_id}`);

    // Study loop
    for (let i = 0; i < 5; i++) { // Study 5 cards
      // Get next card
      const cardResponse = await cli.getNextCard(session.session_id);

      if (!cardResponse.has_more) {
        console.log('No more cards available');
        break;
      }

      // Get card content
      const note = await cli.getNote(deck.id, cardResponse.card.note_id);
      console.log(`Card ${i + 1}: ${note.note.fields[0]} → ${note.note.fields[2]}`);

      // Simulate user rating (in real app, this comes from UI)
      const ratings = ['good', 'easy', 'hard'];
      const rating = ratings[Math.floor(Math.random() * ratings.length)];

      // Rate the card
      await cli.rateCard(session.session_id, cardResponse.card.id, rating);
      console.log(`Rated as: ${rating}`);
    }

    // End session
    await cli.endSession(session.session_id);
    console.log('Session completed');

  } catch (error) {
    console.error('Study session failed:', error.message);
  }
}

// Run the example
studySession();
```

#### Python Example
```python
import subprocess
import json
import random

class CardBrickCLI:
    def __init__(self, cli_path='./target/release/cardbrick-cli'):
        self.cli_path = cli_path

    def run_command(self, args):
        """Run a CLI command and return the JSON response"""
        try:
            result = subprocess.run(
                [self.cli_path] + args.split(),
                capture_output=True,
                text=True,
                check=True
            )
            return json.loads(result.stdout)
        except subprocess.CalledProcessError as e:
            error_data = json.loads(e.stdout) if e.stdout else {"error": True, "message": e.stderr}
            raise Exception(f"CLI Error: {error_data.get('message', 'Unknown error')}")
        except json.JSONDecodeError as e:
            raise Exception(f"Invalid JSON response: {e}")

    def list_decks(self):
        return self.run_command('deck list')

    def start_session(self, deck_id):
        return self.run_command(f'session start {deck_id}')

    def get_next_card(self, session_id):
        return self.run_command(f'session next {session_id}')

    def get_note(self, deck_id, note_id):
        return self.run_command(f'deck note {deck_id} {note_id}')

    def rate_card(self, session_id, card_id, rating):
        return self.run_command(f'session rate {session_id} {card_id} {rating}')

    def end_session(self, session_id):
        return self.run_command(f'session end {session_id}')

# Example usage
def main():
    cli = CardBrickCLI()

    try:
        # Get available decks
        decks = cli.list_decks()
        if not decks['decks']:
            print("No decks available")
            return

        deck = decks['decks'][0]
        print(f"Using deck: {deck['name']}")

        # Start session
        session = cli.start_session(deck['id'])
        session_id = session['session_id']
        print(f"Started session: {session_id}")

        # Study loop
        cards_studied = 0
        while cards_studied < 5:  # Study 5 cards
            # Get next card
            card_response = cli.get_next_card(session_id)

            if not card_response['has_more']:
                print("No more cards available")
                break

            # Get card content
            note = cli.get_note(deck['id'], card_response['card']['note_id'])
            fields = note['note']['fields']
            print(f"Card {cards_studied + 1}: {fields[0]} → {fields[-1]}")

            # Simulate user rating
            rating = random.choice(['good', 'easy', 'hard'])

            # Rate the card
            cli.rate_card(session_id, card_response['card']['id'], rating)
            print(f"Rated as: {rating}")

            cards_studied += 1

        # End session
        cli.end_session(session_id)
        print("Session completed")

    except Exception as e:
        print(f"Error: {e}")

if __name__ == "__main__":
    main()
```

---

## Command Reference

### Deck Commands

#### `deck list`
**Purpose:** Get all available cached decks

**Usage:**
```bash
cardbrick-cli deck list
```

**Response:**
```json
{
  "decks": [
    {
      "id": "deck-hash",
      "name": "Deck Name",
      "path": "/path/to/deck"
    }
  ]
}
```

**When to use:**
- At app startup to populate deck selection
- To refresh available decks

**Error conditions:**
- No cached decks available (returns empty array)
- Cache directory doesn't exist (returns empty array)

---

#### `deck info <deck_id>`
**Purpose:** Get detailed information about a specific deck

**Usage:**
```bash
cardbrick-cli deck info 7766cd891981a68c147e026fdef1520548f9fed81618c0b114749bd7eeacb0d6
```

**Parameters:**
- `deck_id`: The SHA256 hash identifier from `deck list`

**Response:**
```json
{
  "id": "deck-hash",
  "name": "Deck Name",
  "path": "/path/to/deck",
  "manifest": {
    "card_count": 2500,
    "notes_count": 2500,
    "deck_name": "Deck Name",
    "created_at": "2024-01-01T00:00:00Z"
  }
}
```

**When to use:**
- Before starting a session to show deck statistics
- To display deck information in UI

**Error conditions:**
- Deck ID not found: `{"error": true, "message": "Deck not found"}`

---

#### `deck cards <deck_id> [--limit N]`
**Purpose:** Get cards from a deck (for preview/debugging)

**Usage:**
```bash
# Get first 10 cards (default)
cardbrick-cli deck cards 7766cd891981a68c147e026fdef1520548f9fed81618c0b114749bd7eeacb0d6

# Get first 5 cards
cardbrick-cli deck cards 7766cd891981a68c147e026fdef1520548f9fed81618c0b114749bd7eeacb0d6 --limit 5
```

**Parameters:**
- `deck_id`: Deck identifier
- `--limit N`: Number of cards to return (default: 10)

**Response:**
```json
{
  "deck_id": "deck-hash",
  "cards": [
    {
      "id": 1286040963115,
      "note_id": 1286040963115,
      "due": 3412,
      "interval": 0,
      "ease_factor": 2500,
      "lapses": 0
    }
  ],
  "count": 1
}
```

**When to use:**
- Debugging deck content
- Building deck preview features
- **NOT for study sessions** (use session commands instead)

---

#### `deck note <deck_id> <note_id>`
**Purpose:** Get the content (fields) for a specific note

**Usage:**
```bash
cardbrick-cli deck note 7766cd891981a68c147e026fdef1520548f9fed81618c0b114749bd7eeacb0d6 1286040963115
```

**Parameters:**
- `deck_id`: Deck identifier
- `note_id`: Note identifier (from card.note_id)

**Response:**
```json
{
  "deck_id": "deck-hash",
  "note": {
    "id": 1286040963115,
    "fields": [
      "現像",
      "げんぞう",
      "developing (film)"
    ]
  }
}
```

**When to use:**
- **Essential for study sessions** - get card content to display to user
- Building card preview features

**Error conditions:**
- Note not found: `{"deck_id": "...", "note": null}`

---

### Session Commands

#### `session start <deck_id>`
**Purpose:** Start a new study session

**Usage:**
```bash
cardbrick-cli session start 7766cd891981a68c147e026fdef1520548f9fed81618c0b114749bd7eeacb0d6
```

**Parameters:**
- `deck_id`: Deck to study

**Response:**
```json
{
  "session_id": "uuid-string",
  "deck_id": "deck-hash",
  "created_at": "2025-09-23T10:30:00Z",
  "status": "started"
}
```

**Important Notes:**
- **Save the session_id** - you need it for all subsequent operations
- Session state is persistent across CLI calls
- Only one session per session_id

**When to use:**
- When user selects "Study" for a deck
- At the beginning of any study workflow

---

#### `session next <session_id>`
**Purpose:** Get the next card to study

**Usage:**
```bash
cardbrick-cli session next abc123-def456-ghi789
```

**Parameters:**
- `session_id`: From `session start`

**Response (has card):**
```json
{
  "session_id": "uuid-string",
  "card": {
    "id": 1286040963115,
    "note_id": 1286040963115,
    "due": 3412,
    "interval": 0,
    "ease_factor": 2500,
    "lapses": 0
  },
  "card_status": "new",
  "has_more": true
}
```

**Response (no more cards):**
```json
{
  "session_id": "uuid-string",
  "card": null,
  "has_more": false,
  "session_complete": true
}
```

**Card Status Values:**
- `"new"`: First time seeing this card
- `"due"`: Card is due for review
- `"future"`: Card not due yet (only shown if no other cards)

**When to use:**
- Start of study session to get first card
- After rating a card to get the next one
- **Check has_more** to know when session is complete

**Error conditions:**
- Invalid session_id: `{"error": true, "message": "Failed to read session file"}`

---

#### `session rate <session_id> <card_id> <rating>`
**Purpose:** Rate a card and apply spaced repetition algorithm

**Usage:**
```bash
cardbrick-cli session rate abc123-def456-ghi789 1286040963115 good
```

**Parameters:**
- `session_id`: Active session
- `card_id`: Card being rated (from `session next`)
- `rating`: One of `again`, `hard`, `good`, `easy`

**Rating Meanings:**
- `again`: "I didn't know this" - card will be shown again soon
- `hard`: "I struggled with this" - shorter interval than normal
- `good`: "I knew this with some effort" - normal interval
- `easy`: "This was easy" - longer interval than normal

**Response:**
```json
{
  "session_id": "uuid-string",
  "card_id": 1286040963115,
  "rating": "good",
  "cards_studied": 3,
  "status": "rated",
  "timestamp": 1695509972
}
```

**What happens internally:**
1. SM-2 algorithm updates card's interval, ease factor, etc.
2. Card is scheduled for future review
3. Progress is tracked in daily statistics
4. Session counter is incremented

**When to use:**
- **After user reviews a card** and selects their performance
- This is the core of the spaced repetition system

**Error conditions:**
- Invalid rating: `{"error": true, "message": "Invalid rating: xyz. Must be one of: again, hard, good, easy"}`
- Invalid session/card: `{"error": true, "message": "Failed to read session file"}`

---

#### `session status <session_id>`
**Purpose:** Get current session information

**Usage:**
```bash
cardbrick-cli session status abc123-def456-ghi789
```

**Response:**
```json
{
  "session_id": "uuid-string",
  "deck_id": "deck-hash",
  "created_at": "2025-09-23T10:30:00Z",
  "cards_studied": 5,
  "current_card": {
    "id": 1286040963120,
    "note_id": 1286040963120,
    "due": 3417,
    "interval": 0,
    "ease_factor": 2500,
    "lapses": 0
  }
}
```

**When to use:**
- To display session progress in UI
- To resume a session after app restart
- For debugging session state

---

#### `session end <session_id>`
**Purpose:** End a study session and clean up

**Usage:**
```bash
cardbrick-cli session end abc123-def456-ghi789
```

**Response:**
```json
{
  "session_id": "uuid-string",
  "cards_studied": 10,
  "duration_minutes": 15,
  "status": "ended"
}
```

**What happens:**
- Session file is deleted
- Final statistics are returned
- No more operations possible with this session_id

**When to use:**
- When user finishes studying
- When user quits the app
- **Important:** Always end sessions to clean up temporary files

---

### Card Commands

#### `card get <deck_id> <card_id>`
**Purpose:** Get complete card and note information

**Usage:**
```bash
cardbrick-cli card get 7766cd891981a68c147e026fdef1520548f9fed81618c0b114749bd7eeacb0d6 1286040963115
```

**Response:**
```json
{
  "deck_id": "deck-hash",
  "card": {
    "id": 1286040963115,
    "note_id": 1286040963115,
    "due": 3412,
    "interval": 0,
    "ease_factor": 2500,
    "lapses": 0
  },
  "note": {
    "id": 1286040963115,
    "fields": [
      "現像",
      "げんぞう",
      "developing (film)"
    ]
  }
}
```

**When to use:**
- When you need both card scheduling info AND content
- For card detail views
- Alternative to separate `deck note` call

---

#### `card rate <card_id> <rating>`
**Purpose:** Rate a card outside of a session

**Usage:**
```bash
cardbrick-cli card rate 1286040963115 good
```

**Response:**
```json
{
  "card_id": 1286040963115,
  "rating": "good",
  "timestamp": 1695509972,
  "status": "rated",
  "srs_state": {
    "interval": 6,
    "ease_factor": 2.5,
    "repetitions": 2,
    "lapses": 0
  }
}
```

**When to use:**
- **Advanced use cases** where you manage your own session logic
- Bulk operations or automated testing
- **Prefer session-based rating for normal study flows**

---

#### `card history <card_id>`
**Purpose:** Get complete review history for a card

**Usage:**
```bash
cardbrick-cli card history 1286040963115
```

**Response:**
```json
{
  "card_id": 1286040963115,
  "history": [
    {
      "rating": "good",
      "timestamp": 1695509972,
      "date": "2025-09-23",
      "source": "daily_ratings"
    },
    {
      "id": 1695409972,
      "card_id": 1286040963115,
      "ease": 3,
      "interval": 6,
      "factor": 2500,
      "time_taken": 3000,
      "source": "revlog"
    }
  ],
  "total_reviews": 2
}
```

**History Sources:**
- `daily_ratings`: Reviews from CLI/current system
- `revlog`: Reviews from original Anki data

**When to use:**
- Card analytics and review analysis
- Debugging spaced repetition behavior
- Building progress visualization

---

### Progress Commands

#### `progress daily`
**Purpose:** Get today's study progress

**Usage:**
```bash
cardbrick-cli progress daily
```

**Response:**
```json
{
  "date": "2025-09-23",
  "ratings": [
    [1286040963115, "good"],
    [1286040963116, "hard"],
    [1286040963117, "easy"]
  ],
  "total_points": 45,
  "cards_studied": 3
}
```

**When to use:**
- Daily progress dashboard
- Showing today's accomplishments
- Progress visualization

---

#### `progress streak`
**Purpose:** Get daily streak and profile information

**Usage:**
```bash
cardbrick-cli progress streak
```

**Response:**
```json
{
  "daily_streak": 7,
  "last_study_date": "2025-09-22",
  "total_score": 1250,
  "level_score": 250
}
```

**When to use:**
- Profile/dashboard display
- Motivation features
- Long-term progress tracking

---

#### `progress points [--date YYYY-MM-DD]`
**Purpose:** Get points for a specific date

**Usage:**
```bash
# Today's points
cardbrick-cli progress points

# Specific date
cardbrick-cli progress points --date 2025-09-20
```

**Response:**
```json
{
  "date": "2025-09-23",
  "total_points": 45
}
```

**When to use:**
- Historical progress analysis
- Building point charts/graphs
- Progress tracking over time

---

#### `progress difficult`
**Purpose:** Get today's difficult cards (rated Hard or Again)

**Usage:**
```bash
cardbrick-cli progress difficult
```

**Response:**
```json
{
  "date": "2025-09-23",
  "failed_cards": [1286040963118],
  "hard_cards": [1286040963116, 1286040963119],
  "total_difficult": 3
}
```

**When to use:**
- "Review difficult cards" feature
- Identifying problem areas
- Additional practice sessions

---

### Statistics Commands

#### `stats profile`
**Purpose:** Get comprehensive user profile statistics

**Usage:**
```bash
cardbrick-cli stats profile
```

**Response:**
```json
{
  "total_score": 1250,
  "level_score": 250,
  "daily_streak": 7,
  "last_study_date": "2025-09-22",
  "points_today": 45,
  "total_srs_cards": 1847
}
```

**When to use:**
- User profile display
- Main dashboard
- Overall progress overview

---

#### `stats deck <deck_id>`
**Purpose:** Get statistics for a specific deck

**Usage:**
```bash
cardbrick-cli stats deck 7766cd891981a68c147e026fdef1520548f9fed81618c0b114749bd7eeacb0d6
```

**Response:**
```json
{
  "deck_id": "deck-hash",
  "total_cards": 2500,
  "srs_statistics": {
    "total_in_srs": 847,
    "avg_ease_factor": 2.3,
    "avg_interval": 8.5,
    "total_lapses": 23
  }
}
```

**When to use:**
- Deck-specific analytics
- Choosing which deck to study
- Progress tracking per deck

---

#### `stats points [--date YYYY-MM-DD]`
**Purpose:** Get detailed point breakdown

**Usage:**
```bash
cardbrick-cli stats points --date 2025-09-23
```

**Response:**
```json
{
  "date": "2025-09-23",
  "breakdown": {
    "total_events": 5,
    "base_points": 50,
    "difficulty_bonus": 15,
    "combo_bonus": 8,
    "speed_bonus": 12,
    "total_points": 85
  }
}
```

**When to use:**
- Detailed analytics
- Understanding point calculation
- Optimizing study performance

---

## Error Handling

### Understanding Error Responses

All commands return JSON. Errors have this format:
```json
{
  "error": true,
  "message": "Description of what went wrong"
}
```

### Common Error Types

#### 1. Command Not Found
```bash
# Wrong command
./target/release/cardbrick-cli invalid-command
```
**Error:** CLI exits with help message

**Solution:** Check command spelling, use `--help`

#### 2. Invalid Arguments
```bash
# Missing required argument
./target/release/cardbrick-cli deck info
```
**Error:** CLI shows usage help

**Solution:** Provide required arguments

#### 3. Deck Not Found
```bash
./target/release/cardbrick-cli deck info invalid-deck-id
```
**Response:**
```json
{
  "error": true,
  "message": "Deck not found"
}
```

**Common causes:**
- Typo in deck ID
- Deck cache not set up
- Deck removed from cache

#### 4. Session Not Found
```bash
./target/release/cardbrick-cli session next invalid-session-id
```
**Response:**
```json
{
  "error": true,
  "message": "Failed to read session file"
}
```

**Common causes:**
- Session ID typo
- Session expired/deleted
- Session ended

#### 5. Invalid Rating
```bash
./target/release/cardbrick-cli card rate 123 "terrible"
```
**Response:**
```json
{
  "error": true,
  "message": "Invalid rating: terrible. Must be one of: again, hard, good, easy"
}
```

#### 6. Database Issues
**Response:**
```json
{
  "error": true,
  "message": "Failed to open database: No such file or directory"
}
```

**Common causes:**
- No cached decks
- Permissions issue
- Corrupted cache

### Error Handling Best Practices

#### 1. Always Check for Errors
```javascript
// Bad
const result = await runCLI('deck list');
const decks = result.decks; // Crashes if error

// Good
const result = await runCLI('deck list');
if (result.error) {
  console.error('Failed to list decks:', result.message);
  return;
}
const decks = result.decks;
```

#### 2. Handle Specific Error Types
```javascript
async function startSession(deckId) {
  const result = await runCLI(`session start ${deckId}`);

  if (result.error) {
    if (result.message.includes('Deck not found')) {
      throw new Error('Selected deck is no longer available');
    } else if (result.message.includes('Failed to create')) {
      throw new Error('Unable to start session - check permissions');
    } else {
      throw new Error(`Session start failed: ${result.message}`);
    }
  }

  return result;
}
```

#### 3. Graceful Degradation
```javascript
async function getProgress() {
  try {
    const progress = await runCLI('progress daily');
    return progress;
  } catch (error) {
    console.warn('Could not load progress:', error.message);
    // Return default/empty progress
    return {
      date: new Date().toISOString().split('T')[0],
      ratings: [],
      total_points: 0,
      cards_studied: 0
    };
  }
}
```

#### 4. Retry Logic for Transient Errors
```javascript
async function runCLIWithRetry(command, maxRetries = 3) {
  for (let i = 0; i < maxRetries; i++) {
    try {
      return await runCLI(command);
    } catch (error) {
      if (i === maxRetries - 1) throw error;

      // Wait before retry
      await new Promise(resolve => setTimeout(resolve, 1000 * (i + 1)));
    }
  }
}
```

---

## Integration Examples

### React Frontend Example

```jsx
import React, { useState, useEffect } from 'react';
import { exec } from 'child_process';
import { promisify } from 'util';

const execAsync = promisify(exec);

// CLI wrapper hook
function useCardBrickCLI() {
  const runCommand = async (args) => {
    try {
      const { stdout, stderr } = await execAsync(`./target/release/cardbrick-cli ${args}`);
      if (stderr) throw new Error(stderr);
      return JSON.parse(stdout);
    } catch (error) {
      throw new Error(`CLI Error: ${error.message}`);
    }
  };

  return {
    listDecks: () => runCommand('deck list'),
    startSession: (deckId) => runCommand(`session start ${deckId}`),
    getNextCard: (sessionId) => runCommand(`session next ${sessionId}`),
    getNote: (deckId, noteId) => runCommand(`deck note ${deckId} ${noteId}`),
    rateCard: (sessionId, cardId, rating) => runCommand(`session rate ${sessionId} ${cardId} ${rating}`),
    endSession: (sessionId) => runCommand(`session end ${sessionId}`)
  };
}

// Study Session Component
function StudySession({ deckId, onComplete }) {
  const [sessionId, setSessionId] = useState(null);
  const [currentCard, setCurrentCard] = useState(null);
  const [noteContent, setNoteContent] = useState(null);
  const [showAnswer, setShowAnswer] = useState(false);
  const [cardsStudied, setCardsStudied] = useState(0);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState(null);

  const cli = useCardBrickCLI();

  // Start session on mount
  useEffect(() => {
    startSession();
  }, [deckId]);

  const startSession = async () => {
    try {
      setIsLoading(true);
      const session = await cli.startSession(deckId);
      setSessionId(session.session_id);
      await loadNextCard(session.session_id);
    } catch (err) {
      setError(`Failed to start session: ${err.message}`);
    } finally {
      setIsLoading(false);
    }
  };

  const loadNextCard = async (currentSessionId) => {
    try {
      setIsLoading(true);
      const cardResponse = await cli.getNextCard(currentSessionId);

      if (!cardResponse.has_more) {
        await cli.endSession(currentSessionId);
        onComplete();
        return;
      }

      setCurrentCard(cardResponse.card);

      // Load note content
      const note = await cli.getNote(deckId, cardResponse.card.note_id);
      setNoteContent(note.note);
      setShowAnswer(false);
    } catch (err) {
      setError(`Failed to load card: ${err.message}`);
    } finally {
      setIsLoading(false);
    }
  };

  const rateCard = async (rating) => {
    try {
      setIsLoading(true);
      await cli.rateCard(sessionId, currentCard.id, rating);
      setCardsStudied(prev => prev + 1);
      await loadNextCard(sessionId);
    } catch (err) {
      setError(`Failed to rate card: ${err.message}`);
    } finally {
      setIsLoading(false);
    }
  };

  if (isLoading) return <div>Loading...</div>;
  if (error) return <div>Error: {error}</div>;
  if (!currentCard || !noteContent) return <div>No card loaded</div>;

  return (
    <div className="study-session">
      <div className="progress">Cards studied: {cardsStudied}</div>

      <div className="card">
        <div className="question">
          {noteContent.fields[0]}
        </div>

        {showAnswer && (
          <div className="answer">
            {noteContent.fields[2]}
          </div>
        )}
      </div>

      {!showAnswer ? (
        <button onClick={() => setShowAnswer(true)}>
          Show Answer
        </button>
      ) : (
        <div className="rating-buttons">
          <button onClick={() => rateCard('again')}>Again</button>
          <button onClick={() => rateCard('hard')}>Hard</button>
          <button onClick={() => rateCard('good')}>Good</button>
          <button onClick={() => rateCard('easy')}>Easy</button>
        </div>
      )}
    </div>
  );
}

// Main App Component
function App() {
  const [decks, setDecks] = useState([]);
  const [selectedDeck, setSelectedDeck] = useState(null);
  const [isStudying, setIsStudying] = useState(false);

  const cli = useCardBrickCLI();

  useEffect(() => {
    loadDecks();
  }, []);

  const loadDecks = async () => {
    try {
      const deckList = await cli.listDecks();
      setDecks(deckList.decks);
    } catch (err) {
      console.error('Failed to load decks:', err);
    }
  };

  const startStudying = (deck) => {
    setSelectedDeck(deck);
    setIsStudying(true);
  };

  const finishStudying = () => {
    setIsStudying(false);
    setSelectedDeck(null);
  };

  if (isStudying) {
    return (
      <StudySession
        deckId={selectedDeck.id}
        onComplete={finishStudying}
      />
    );
  }

  return (
    <div className="app">
      <h1>CardBrick Study App</h1>
      <div className="deck-list">
        {decks.map(deck => (
          <div key={deck.id} className="deck-item">
            <h3>{deck.name}</h3>
            <button onClick={() => startStudying(deck)}>
              Study
            </button>
          </div>
        ))}
      </div>
    </div>
  );
}

export default App;
```

### Vue.js Frontend Example

```vue
<template>
  <div class="cardbrick-app">
    <div v-if="!isStudying" class="deck-selection">
      <h1>Select a Deck to Study</h1>
      <div v-for="deck in decks" :key="deck.id" class="deck-card">
        <h3>{{ deck.name }}</h3>
        <button @click="startStudying(deck)" :disabled="loading">
          Study This Deck
        </button>
      </div>
    </div>

    <div v-else class="study-session">
      <div class="session-header">
        <h2>{{ selectedDeck.name }}</h2>
        <p>Cards studied: {{ cardsStudied }}</p>
        <button @click="endStudySession">End Session</button>
      </div>

      <div v-if="currentCard" class="flashcard">
        <div class="card-front">
          {{ noteContent?.fields[0] }}
        </div>

        <div v-if="showAnswer" class="card-back">
          {{ noteContent?.fields[2] }}
        </div>

        <div v-if="!showAnswer" class="show-answer">
          <button @click="showAnswer = true" :disabled="loading">
            Show Answer
          </button>
        </div>

        <div v-else class="rating-buttons">
          <button @click="rateCard('again')" :disabled="loading">
            Again
          </button>
          <button @click="rateCard('hard')" :disabled="loading">
            Hard
          </button>
          <button @click="rateCard('good')" :disabled="loading">
            Good
          </button>
          <button @click="rateCard('easy')" :disabled="loading">
            Easy
          </button>
        </div>
      </div>

      <div v-if="loading" class="loading">
        Processing...
      </div>
    </div>

    <div v-if="error" class="error">
      {{ error }}
      <button @click="error = null">Dismiss</button>
    </div>
  </div>
</template>

<script>
import { ref, onMounted } from 'vue'
import { exec } from 'child_process'
import { promisify } from 'util'

const execAsync = promisify(exec)

export default {
  name: 'CardBrickApp',
  setup() {
    const decks = ref([])
    const selectedDeck = ref(null)
    const isStudying = ref(false)
    const sessionId = ref(null)
    const currentCard = ref(null)
    const noteContent = ref(null)
    const showAnswer = ref(false)
    const cardsStudied = ref(0)
    const loading = ref(false)
    const error = ref(null)

    // CLI wrapper
    const runCLI = async (command) => {
      try {
        const { stdout, stderr } = await execAsync(`./target/release/cardbrick-cli ${command}`)
        if (stderr) throw new Error(stderr)
        const result = JSON.parse(stdout)
        if (result.error) throw new Error(result.message)
        return result
      } catch (err) {
        throw new Error(`CLI Error: ${err.message}`)
      }
    }

    // Load available decks
    const loadDecks = async () => {
      try {
        loading.value = true
        const deckList = await runCLI('deck list')
        decks.value = deckList.decks
      } catch (err) {
        error.value = `Failed to load decks: ${err.message}`
      } finally {
        loading.value = false
      }
    }

    // Start studying a deck
    const startStudying = async (deck) => {
      try {
        loading.value = true
        selectedDeck.value = deck
        isStudying.value = true
        cardsStudied.value = 0

        // Start session
        const session = await runCLI(`session start ${deck.id}`)
        sessionId.value = session.session_id

        // Load first card
        await loadNextCard()
      } catch (err) {
        error.value = `Failed to start session: ${err.message}`
        isStudying.value = false
      } finally {
        loading.value = false
      }
    }

    // Load the next card
    const loadNextCard = async () => {
      try {
        loading.value = true

        // Get next card
        const cardResponse = await runCLI(`session next ${sessionId.value}`)

        if (!cardResponse.has_more) {
          // Session complete
          await runCLI(`session end ${sessionId.value}`)
          alert(`Session complete! You studied ${cardsStudied.value} cards.`)
          isStudying.value = false
          return
        }

        currentCard.value = cardResponse.card
        showAnswer.value = false

        // Load note content
        const note = await runCLI(`deck note ${selectedDeck.value.id} ${cardResponse.card.note_id}`)
        noteContent.value = note.note
      } catch (err) {
        error.value = `Failed to load card: ${err.message}`
      } finally {
        loading.value = false
      }
    }

    // Rate the current card
    const rateCard = async (rating) => {
      try {
        loading.value = true

        await runCLI(`session rate ${sessionId.value} ${currentCard.value.id} ${rating}`)
        cardsStudied.value++

        // Load next card
        await loadNextCard()
      } catch (err) {
        error.value = `Failed to rate card: ${err.message}`
      } finally {
        loading.value = false
      }
    }

    // End study session
    const endStudySession = async () => {
      try {
        if (sessionId.value) {
          await runCLI(`session end ${sessionId.value}`)
        }
        isStudying.value = false
        selectedDeck.value = null
        sessionId.value = null
        currentCard.value = null
        noteContent.value = null
      } catch (err) {
        error.value = `Failed to end session: ${err.message}`
      }
    }

    // Load decks on mount
    onMounted(() => {
      loadDecks()
    })

    return {
      decks,
      selectedDeck,
      isStudying,
      currentCard,
      noteContent,
      showAnswer,
      cardsStudied,
      loading,
      error,
      startStudying,
      rateCard,
      endStudySession
    }
  }
}
</script>

<style scoped>
.cardbrick-app {
  max-width: 800px;
  margin: 0 auto;
  padding: 20px;
}

.deck-card {
  border: 1px solid #ccc;
  padding: 15px;
  margin: 10px 0;
  border-radius: 5px;
}

.flashcard {
  border: 2px solid #333;
  padding: 30px;
  margin: 20px 0;
  border-radius: 10px;
  text-align: center;
  min-height: 200px;
}

.card-front {
  font-size: 24px;
  font-weight: bold;
  margin-bottom: 20px;
}

.card-back {
  font-size: 20px;
  color: #666;
  margin-bottom: 20px;
}

.rating-buttons {
  display: flex;
  gap: 10px;
  justify-content: center;
}

.rating-buttons button {
  padding: 10px 20px;
  font-size: 16px;
  border: none;
  border-radius: 5px;
  cursor: pointer;
}

.rating-buttons button:nth-child(1) { background-color: #ff6b6b; }
.rating-buttons button:nth-child(2) { background-color: #feca57; }
.rating-buttons button:nth-child(3) { background-color: #48dbfb; }
.rating-buttons button:nth-child(4) { background-color: #0be881; }

.error {
  background-color: #ff6b6b;
  color: white;
  padding: 15px;
  border-radius: 5px;
  margin: 10px 0;
}

.loading {
  text-align: center;
  font-style: italic;
  color: #666;
}
</style>
```

### Python Flask Web App Example

```python
from flask import Flask, render_template, request, jsonify, session
import subprocess
import json
import uuid
from datetime import datetime

app = Flask(__name__)
app.secret_key = 'your-secret-key-here'

class CardBrickCLI:
    def __init__(self, cli_path='./target/release/cardbrick-cli'):
        self.cli_path = cli_path

    def run_command(self, args):
        try:
            result = subprocess.run(
                [self.cli_path] + args.split(),
                capture_output=True,
                text=True,
                check=True
            )
            data = json.loads(result.stdout)
            if data.get('error'):
                raise Exception(data.get('message', 'Unknown error'))
            return data
        except subprocess.CalledProcessError as e:
            if e.stdout:
                error_data = json.loads(e.stdout)
                raise Exception(error_data.get('message', 'CLI command failed'))
            raise Exception(f"CLI error: {e.stderr}")
        except json.JSONDecodeError:
            raise Exception("Invalid CLI response")

cli = CardBrickCLI()

@app.route('/')
def index():
    return render_template('index.html')

@app.route('/api/decks')
def list_decks():
    try:
        decks = cli.run_command('deck list')
        return jsonify(decks)
    except Exception as e:
        return jsonify({'error': True, 'message': str(e)}), 500

@app.route('/api/session/start', methods=['POST'])
def start_session():
    try:
        deck_id = request.json.get('deck_id')
        if not deck_id:
            return jsonify({'error': True, 'message': 'deck_id required'}), 400

        session_data = cli.run_command(f'session start {deck_id}')

        # Store session info in Flask session
        session['session_id'] = session_data['session_id']
        session['deck_id'] = deck_id
        session['cards_studied'] = 0

        return jsonify(session_data)
    except Exception as e:
        return jsonify({'error': True, 'message': str(e)}), 500

@app.route('/api/session/next')
def next_card():
    try:
        session_id = session.get('session_id')
        if not session_id:
            return jsonify({'error': True, 'message': 'No active session'}), 400

        card_data = cli.run_command(f'session next {session_id}')

        if card_data['has_more']:
            # Get note content
            deck_id = session.get('deck_id')
            note_data = cli.run_command(f'deck note {deck_id} {card_data["card"]["note_id"]}')
            card_data['note'] = note_data['note']

        return jsonify(card_data)
    except Exception as e:
        return jsonify({'error': True, 'message': str(e)}), 500

@app.route('/api/session/rate', methods=['POST'])
def rate_card():
    try:
        session_id = session.get('session_id')
        if not session_id:
            return jsonify({'error': True, 'message': 'No active session'}), 400

        card_id = request.json.get('card_id')
        rating = request.json.get('rating')

        if not card_id or not rating:
            return jsonify({'error': True, 'message': 'card_id and rating required'}), 400

        result = cli.run_command(f'session rate {session_id} {card_id} {rating}')

        # Update session tracking
        session['cards_studied'] = result['cards_studied']

        return jsonify(result)
    except Exception as e:
        return jsonify({'error': True, 'message': str(e)}), 500

@app.route('/api/session/end', methods=['POST'])
def end_session():
    try:
        session_id = session.get('session_id')
        if not session_id:
            return jsonify({'error': True, 'message': 'No active session'}), 400

        result = cli.run_command(f'session end {session_id}')

        # Clear session data
        session.pop('session_id', None)
        session.pop('deck_id', None)
        session.pop('cards_studied', None)

        return jsonify(result)
    except Exception as e:
        return jsonify({'error': True, 'message': str(e)}), 500

@app.route('/api/progress/daily')
def daily_progress():
    try:
        progress = cli.run_command('progress daily')
        return jsonify(progress)
    except Exception as e:
        return jsonify({'error': True, 'message': str(e)}), 500

@app.route('/api/stats/profile')
def profile_stats():
    try:
        stats = cli.run_command('stats profile')
        return jsonify(stats)
    except Exception as e:
        return jsonify({'error': True, 'message': str(e)}), 500

if __name__ == '__main__':
    app.run(debug=True)
```

```html
<!-- templates/index.html -->
<!DOCTYPE html>
<html>
<head>
    <title>CardBrick Web App</title>
    <style>
        body { font-family: Arial, sans-serif; max-width: 800px; margin: 0 auto; padding: 20px; }
        .deck-list { display: grid; gap: 15px; }
        .deck-item { border: 1px solid #ccc; padding: 15px; border-radius: 5px; }
        .flashcard { border: 2px solid #333; padding: 40px; margin: 20px 0; border-radius: 10px; text-align: center; }
        .rating-buttons { display: flex; gap: 10px; justify-content: center; margin-top: 20px; }
        .rating-buttons button { padding: 10px 20px; border: none; border-radius: 5px; cursor: pointer; }
        .hidden { display: none; }
        .error { background-color: #ff6b6b; color: white; padding: 15px; border-radius: 5px; margin: 10px 0; }
    </style>
</head>
<body>
    <div id="app">
        <!-- Deck Selection -->
        <div id="deck-selection">
            <h1>CardBrick Study App</h1>
            <div id="deck-list" class="deck-list"></div>
        </div>

        <!-- Study Session -->
        <div id="study-session" class="hidden">
            <div id="session-header">
                <h2 id="deck-name"></h2>
                <p>Cards studied: <span id="cards-studied">0</span></p>
                <button onclick="endSession()">End Session</button>
            </div>

            <div id="flashcard" class="flashcard">
                <div id="card-front"></div>
                <div id="card-back" class="hidden"></div>

                <button id="show-answer-btn" onclick="showAnswer()">Show Answer</button>

                <div id="rating-buttons" class="rating-buttons hidden">
                    <button onclick="rateCard('again')" style="background-color: #ff6b6b;">Again</button>
                    <button onclick="rateCard('hard')" style="background-color: #feca57;">Hard</button>
                    <button onclick="rateCard('good')" style="background-color: #48dbfb;">Good</button>
                    <button onclick="rateCard('easy')" style="background-color: #0be881;">Easy</button>
                </div>
            </div>
        </div>

        <div id="error-message" class="error hidden"></div>
    </div>

    <script>
        let currentCard = null;
        let sessionActive = false;

        // Load decks on page load
        window.addEventListener('load', loadDecks);

        async function apiCall(url, options = {}) {
            try {
                const response = await fetch(url, {
                    headers: {
                        'Content-Type': 'application/json',
                        ...options.headers
                    },
                    ...options
                });

                const data = await response.json();

                if (data.error) {
                    throw new Error(data.message);
                }

                return data;
            } catch (error) {
                showError(error.message);
                throw error;
            }
        }

        async function loadDecks() {
            try {
                const decks = await apiCall('/api/decks');
                displayDecks(decks.decks);
            } catch (error) {
                console.error('Failed to load decks:', error);
            }
        }

        function displayDecks(decks) {
            const deckList = document.getElementById('deck-list');
            deckList.innerHTML = '';

            decks.forEach(deck => {
                const deckItem = document.createElement('div');
                deckItem.className = 'deck-item';
                deckItem.innerHTML = `
                    <h3>${deck.name}</h3>
                    <button onclick="startStudying('${deck.id}', '${deck.name}')">Study</button>
                `;
                deckList.appendChild(deckItem);
            });
        }

        async function startStudying(deckId, deckName) {
            try {
                await apiCall('/api/session/start', {
                    method: 'POST',
                    body: JSON.stringify({ deck_id: deckId })
                });

                document.getElementById('deck-name').textContent = deckName;
                document.getElementById('deck-selection').classList.add('hidden');
                document.getElementById('study-session').classList.remove('hidden');

                sessionActive = true;
                await loadNextCard();
            } catch (error) {
                console.error('Failed to start session:', error);
            }
        }

        async function loadNextCard() {
            try {
                const cardData = await apiCall('/api/session/next');

                if (!cardData.has_more) {
                    alert('Session complete!');
                    endSession();
                    return;
                }

                currentCard = cardData.card;
                const note = cardData.note;

                document.getElementById('card-front').textContent = note.fields[0];
                document.getElementById('card-back').textContent = note.fields[note.fields.length - 1];

                // Reset card display
                document.getElementById('card-back').classList.add('hidden');
                document.getElementById('show-answer-btn').classList.remove('hidden');
                document.getElementById('rating-buttons').classList.add('hidden');
            } catch (error) {
                console.error('Failed to load card:', error);
            }
        }

        function showAnswer() {
            document.getElementById('card-back').classList.remove('hidden');
            document.getElementById('show-answer-btn').classList.add('hidden');
            document.getElementById('rating-buttons').classList.remove('hidden');
        }

        async function rateCard(rating) {
            try {
                const result = await apiCall('/api/session/rate', {
                    method: 'POST',
                    body: JSON.stringify({
                        card_id: currentCard.id,
                        rating: rating
                    })
                });

                document.getElementById('cards-studied').textContent = result.cards_studied;
                await loadNextCard();
            } catch (error) {
                console.error('Failed to rate card:', error);
            }
        }

        async function endSession() {
            if (sessionActive) {
                try {
                    await apiCall('/api/session/end', { method: 'POST' });
                } catch (error) {
                    console.error('Failed to end session:', error);
                }
            }

            sessionActive = false;
            document.getElementById('study-session').classList.add('hidden');
            document.getElementById('deck-selection').classList.remove('hidden');
            document.getElementById('cards-studied').textContent = '0';
        }

        function showError(message) {
            const errorDiv = document.getElementById('error-message');
            errorDiv.textContent = message;
            errorDiv.classList.remove('hidden');

            setTimeout(() => {
                errorDiv.classList.add('hidden');
            }, 5000);
        }
    </script>
</body>
</html>
```

---

## Testing Guide

### Unit Testing CLI Commands

#### Test Setup
```bash
# Create a test script
cat > test_cli.sh << 'EOF'
#!/bin/bash

CLI_PATH="./target/release/cardbrick-cli"
TEST_DECK_ID="7766cd891981a68c147e026fdef1520548f9fed81618c0b114749bd7eeacb0d6"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
NC='\033[0m' # No Color

# Test counter
TESTS_RUN=0
TESTS_PASSED=0

run_test() {
    local test_name="$1"
    local command="$2"
    local expected_status="$3"  # 0 for success, 1 for failure

    echo "Running test: $test_name"

    TESTS_RUN=$((TESTS_RUN + 1))

    # Run command and capture output
    output=$($CLI_PATH $command 2>&1)
    status=$?

    if [ $status -eq $expected_status ]; then
        echo -e "${GREEN}✓ PASSED${NC}: $test_name"
        TESTS_PASSED=$((TESTS_PASSED + 1))

        # Validate JSON if expecting success
        if [ $expected_status -eq 0 ]; then
            if echo "$output" | python3 -m json.tool > /dev/null 2>&1; then
                echo "  JSON output is valid"
            else
                echo -e "${RED}✗ WARNING${NC}: Output is not valid JSON"
            fi
        fi
    else
        echo -e "${RED}✗ FAILED${NC}: $test_name"
        echo "  Expected status: $expected_status, Got: $status"
        echo "  Output: $output"
    fi

    echo ""
}

echo "Starting CardBrick CLI Tests"
echo "=============================="

# Test basic functionality
run_test "Help command" "--help" 0
run_test "Version command" "--version" 0

# Test deck commands
run_test "List decks" "deck list" 0
run_test "Invalid deck info" "deck info invalid-id" 1
run_test "Get deck cards with limit" "deck cards $TEST_DECK_ID --limit 3" 0

# Test invalid commands
run_test "Invalid command" "invalid-command" 1
run_test "Missing arguments" "deck info" 1

# Test rating validation
run_test "Invalid rating" "card rate 123 invalid-rating" 1

echo "=============================="
echo "Tests completed: $TESTS_RUN"
echo "Tests passed: $TESTS_PASSED"
echo "Tests failed: $((TESTS_RUN - TESTS_PASSED))"

if [ $TESTS_PASSED -eq $TESTS_RUN ]; then
    echo -e "${GREEN}All tests passed!${NC}"
    exit 0
else
    echo -e "${RED}Some tests failed!${NC}"
    exit 1
fi
EOF

chmod +x test_cli.sh
./test_cli.sh
```

#### Integration Test Example
```python
import unittest
import subprocess
import json
import time

class TestCardBrickCLI(unittest.TestCase):
    def setUp(self):
        self.cli_path = './target/release/cardbrick-cli'
        # Use a known test deck ID
        self.test_deck_id = '7766cd891981a68c147e026fdef1520548f9fed81618c0b114749bd7eeacb0d6'

    def run_cli(self, command):
        """Run CLI command and return parsed JSON result"""
        result = subprocess.run(
            [self.cli_path] + command.split(),
            capture_output=True,
            text=True
        )

        # Parse JSON response
        try:
            data = json.loads(result.stdout)
        except json.JSONDecodeError:
            self.fail(f"Invalid JSON response: {result.stdout}")

        return data, result.returncode

    def test_deck_list(self):
        """Test that deck list returns valid data"""
        data, status = self.run_cli('deck list')

        self.assertEqual(status, 0)
        self.assertIn('decks', data)
        self.assertIsInstance(data['decks'], list)

    def test_session_workflow(self):
        """Test complete session workflow"""
        # Start session
        data, status = self.run_cli(f'session start {self.test_deck_id}')
        self.assertEqual(status, 0)
        self.assertIn('session_id', data)

        session_id = data['session_id']

        # Get next card
        data, status = self.run_cli(f'session next {session_id}')
        self.assertEqual(status, 0)
        self.assertTrue(data.get('has_more', False))

        if data['has_more']:
            card = data['card']
            self.assertIn('id', card)
            self.assertIn('note_id', card)

            # Rate the card
            data, status = self.run_cli(f'session rate {session_id} {card["id"]} good')
            self.assertEqual(status, 0)
            self.assertEqual(data['status'], 'rated')

        # End session
        data, status = self.run_cli(f'session end {session_id}')
        self.assertEqual(status, 0)
        self.assertEqual(data['status'], 'ended')

    def test_invalid_session_id(self):
        """Test handling of invalid session ID"""
        data, status = self.run_cli('session next invalid-session-id')

        self.assertEqual(status, 1)
        self.assertTrue(data.get('error', False))
        self.assertIn('message', data)

    def test_card_rating_validation(self):
        """Test card rating validation"""
        # Valid ratings should be accepted
        valid_ratings = ['again', 'hard', 'good', 'easy']

        for rating in valid_ratings:
            data, status = self.run_cli(f'card rate 123 {rating}')
            # This might fail due to card not existing, but rating should be valid
            if data.get('error') and 'Invalid rating' in data.get('message', ''):
                self.fail(f"Valid rating '{rating}' was rejected")

        # Invalid rating should be rejected
        data, status = self.run_cli('card rate 123 invalid')
        self.assertEqual(status, 1)
        self.assertTrue(data.get('error', False))
        self.assertIn('Invalid rating', data.get('message', ''))

if __name__ == '__main__':
    unittest.main()
```

### Performance Testing

#### CLI Response Time Test
```python
import time
import subprocess
import statistics

def measure_cli_performance():
    cli_path = './target/release/cardbrick-cli'

    # Test commands and expected execution times (in seconds)
    test_commands = [
        ('deck list', 0.5),
        ('progress daily', 0.3),
        ('stats profile', 0.3),
    ]

    for command, max_time in test_commands:
        times = []

        # Run command 10 times
        for i in range(10):
            start_time = time.time()

            result = subprocess.run(
                [cli_path] + command.split(),
                capture_output=True,
                text=True
            )

            end_time = time.time()
            execution_time = end_time - start_time
            times.append(execution_time)

            if result.returncode != 0:
                print(f"Command failed: {command}")
                break

        avg_time = statistics.mean(times)
        max_measured = max(times)
        min_measured = min(times)

        print(f"Command: {command}")
        print(f"  Average: {avg_time:.3f}s")
        print(f"  Range: {min_measured:.3f}s - {max_measured:.3f}s")
        print(f"  Target: <{max_time}s")

        if avg_time > max_time:
            print(f"  ⚠️  WARNING: Average time exceeds target")
        else:
            print(f"  ✅ Performance OK")

        print()

if __name__ == '__main__':
    measure_cli_performance()
```

### Load Testing

#### Concurrent Session Test
```python
import concurrent.futures
import subprocess
import json
import time

def test_concurrent_sessions():
    """Test multiple concurrent sessions"""
    cli_path = './target/release/cardbrick-cli'
    test_deck_id = '7766cd891981a68c147e026fdef1520548f9fed81618c0b114749bd7eeacb0d6'

    def run_session(session_num):
        try:
            # Start session
            result = subprocess.run(
                [cli_path, 'session', 'start', test_deck_id],
                capture_output=True,
                text=True,
                check=True
            )

            session_data = json.loads(result.stdout)
            session_id = session_data['session_id']

            # Study a few cards
            for i in range(3):
                # Get next card
                result = subprocess.run(
                    [cli_path, 'session', 'next', session_id],
                    capture_output=True,
                    text=True,
                    check=True
                )

                card_data = json.loads(result.stdout)
                if not card_data['has_more']:
                    break

                # Rate card
                card_id = card_data['card']['id']
                rating = ['good', 'easy'][i % 2]

                subprocess.run(
                    [cli_path, 'session', 'rate', session_id, str(card_id), rating],
                    capture_output=True,
                    text=True,
                    check=True
                )

            # End session
            result = subprocess.run(
                [cli_path, 'session', 'end', session_id],
                capture_output=True,
                text=True,
                check=True
            )

            return f"Session {session_num}: SUCCESS"

        except Exception as e:
            return f"Session {session_num}: FAILED - {e}"

    # Run 5 concurrent sessions
    with concurrent.futures.ThreadPoolExecutor(max_workers=5) as executor:
        futures = [executor.submit(run_session, i) for i in range(5)]

        for future in concurrent.futures.as_completed(futures):
            print(future.result())

if __name__ == '__main__':
    test_concurrent_sessions()
```

---

## Best Practices

### 1. Always Handle Errors Gracefully

```javascript
// Bad: No error handling
async function rateCard(sessionId, cardId, rating) {
    const result = await runCLI(`session rate ${sessionId} ${cardId} ${rating}`);
    return result; // Crashes if CLI returns error
}

// Good: Proper error handling
async function rateCard(sessionId, cardId, rating) {
    try {
        const result = await runCLI(`session rate ${sessionId} ${cardId} ${rating}`);

        if (result.error) {
            throw new Error(result.message);
        }

        return result;
    } catch (error) {
        console.error('Failed to rate card:', error.message);

        // Provide user-friendly error message
        if (error.message.includes('Invalid rating')) {
            throw new Error('Please select a valid rating (Again, Hard, Good, or Easy)');
        } else if (error.message.includes('session')) {
            throw new Error('Study session has expired. Please start a new session.');
        } else {
            throw new Error('Unable to save your rating. Please try again.');
        }
    }
}
```

### 2. Validate Input Before Sending to CLI

```javascript
function validateRating(rating) {
    const validRatings = ['again', 'hard', 'good', 'easy'];
    if (!validRatings.includes(rating.toLowerCase())) {
        throw new Error(`Invalid rating: ${rating}. Must be one of: ${validRatings.join(', ')}`);
    }
    return rating.toLowerCase();
}

function validateSessionId(sessionId) {
    if (!sessionId || typeof sessionId !== 'string' || sessionId.trim() === '') {
        throw new Error('Valid session ID is required');
    }
    return sessionId.trim();
}

// Use validation before CLI calls
async function rateCard(sessionId, cardId, rating) {
    const validSessionId = validateSessionId(sessionId);
    const validRating = validateRating(rating);

    return await runCLI(`session rate ${validSessionId} ${cardId} ${validRating}`);
}
```

### 3. Implement Proper Session Management

```javascript
class StudySessionManager {
    constructor() {
        this.currentSession = null;
        this.isActive = false;
    }

    async startSession(deckId) {
        if (this.isActive) {
            throw new Error('Session already active. End current session first.');
        }

        const session = await runCLI(`session start ${deckId}`);
        this.currentSession = {
            id: session.session_id,
            deckId: deckId,
            startTime: new Date(),
            cardsStudied: 0
        };
        this.isActive = true;

        return session;
    }

    async rateCard(cardId, rating) {
        if (!this.isActive) {
            throw new Error('No active session. Start a session first.');
        }

        const result = await runCLI(`session rate ${this.currentSession.id} ${cardId} ${rating}`);
        this.currentSession.cardsStudied = result.cards_studied;

        return result;
    }

    async endSession() {
        if (!this.isActive) {
            return null; // Already ended
        }

        const result = await runCLI(`session end ${this.currentSession.id}`);
        const sessionSummary = {
            ...this.currentSession,
            endTime: new Date(),
            finalStats: result
        };

        this.currentSession = null;
        this.isActive = false;

        return sessionSummary;
    }

    // Cleanup on page unload
    async cleanup() {
        if (this.isActive) {
            try {
                await this.endSession();
            } catch (error) {
                console.warn('Failed to cleanup session:', error);
            }
        }
    }
}

// Usage
const sessionManager = new StudySessionManager();

// Cleanup on page unload
window.addEventListener('beforeunload', () => {
    sessionManager.cleanup();
});
```

### 4. Cache Data Appropriately

```javascript
class DataCache {
    constructor() {
        this.cache = new Map();
        this.expiry = new Map();
    }

    set(key, value, ttlSeconds = 300) { // 5 minute default TTL
        this.cache.set(key, value);
        this.expiry.set(key, Date.now() + (ttlSeconds * 1000));
    }

    get(key) {
        if (this.expiry.get(key) < Date.now()) {
            this.cache.delete(key);
            this.expiry.delete(key);
            return null;
        }
        return this.cache.get(key);
    }

    clear() {
        this.cache.clear();
        this.expiry.clear();
    }
}

const cache = new DataCache();

async function getDecksWithCache() {
    const cached = cache.get('decks');
    if (cached) {
        return cached;
    }

    const decks = await runCLI('deck list');
    cache.set('decks', decks, 60); // Cache for 1 minute

    return decks;
}

async function getNoteWithCache(deckId, noteId) {
    const cacheKey = `note_${deckId}_${noteId}`;
    const cached = cache.get(cacheKey);
    if (cached) {
        return cached;
    }

    const note = await runCLI(`deck note ${deckId} ${noteId}`);
    cache.set(cacheKey, note, 3600); // Cache for 1 hour

    return note;
}
```

### 5. Implement Retry Logic for Robustness

```javascript
async function runCLIWithRetry(command, maxRetries = 3, baseDelay = 1000) {
    for (let attempt = 1; attempt <= maxRetries; attempt++) {
        try {
            return await runCLI(command);
        } catch (error) {
            console.warn(`CLI command failed (attempt ${attempt}/${maxRetries}):`, error.message);

            if (attempt === maxRetries) {
                throw error; // Final attempt failed
            }

            // Exponential backoff
            const delay = baseDelay * Math.pow(2, attempt - 1);
            await new Promise(resolve => setTimeout(resolve, delay));
        }
    }
}

// Usage
async function rateCardWithRetry(sessionId, cardId, rating) {
    return await runCLIWithRetry(`session rate ${sessionId} ${cardId} ${rating}`);
}
```

### 6. Log Important Operations

```javascript
class CLILogger {
    constructor() {
        this.logs = [];
        this.maxLogs = 1000;
    }

    log(level, command, result, error = null) {
        const logEntry = {
            timestamp: new Date().toISOString(),
            level,
            command,
            result: error ? null : result,
            error: error ? error.message : null
        };

        this.logs.push(logEntry);

        // Keep only recent logs
        if (this.logs.length > this.maxLogs) {
            this.logs.shift();
        }

        // Console logging
        if (level === 'error') {
            console.error('CLI Error:', command, error);
        } else if (level === 'warn') {
            console.warn('CLI Warning:', command, result);
        } else {
            console.log('CLI:', command, result);
        }
    }

    getLogs(level = null) {
        return level ? this.logs.filter(log => log.level === level) : this.logs;
    }

    exportLogs() {
        return JSON.stringify(this.logs, null, 2);
    }
}

const logger = new CLILogger();

async function runCLI(command) {
    try {
        const result = await actualCLICall(command);
        logger.log('info', command, result);
        return result;
    } catch (error) {
        logger.log('error', command, null, error);
        throw error;
    }
}
```

### 7. Handle Background/Concurrent Operations

```javascript
class OperationQueue {
    constructor(maxConcurrent = 3) {
        this.queue = [];
        this.running = [];
        this.maxConcurrent = maxConcurrent;
    }

    async add(operation) {
        return new Promise((resolve, reject) => {
            this.queue.push({
                operation,
                resolve,
                reject
            });
            this.processQueue();
        });
    }

    async processQueue() {
        if (this.running.length >= this.maxConcurrent || this.queue.length === 0) {
            return;
        }

        const item = this.queue.shift();
        this.running.push(item);

        try {
            const result = await item.operation();
            item.resolve(result);
        } catch (error) {
            item.reject(error);
        } finally {
            const index = this.running.indexOf(item);
            if (index > -1) {
                this.running.splice(index, 1);
            }
            this.processQueue(); // Process next item
        }
    }
}

const operationQueue = new OperationQueue(2); // Max 2 concurrent CLI calls

// Queue CLI operations to prevent overwhelming the system
async function queuedCLI(command) {
    return await operationQueue.add(() => runCLI(command));
}
```

---

## Troubleshooting

### Common Issues and Solutions

#### 1. "CLI command not found" or "No such file or directory"

**Problem:** System can't find the CLI binary

**Solutions:**
```bash
# Check if binary exists
ls -la ./target/release/cardbrick-cli

# If not built yet
cargo build --release --bin cardbrick-cli

# Make executable (Linux/Mac)
chmod +x ./target/release/cardbrick-cli

# Use absolute path
/full/path/to/CardBrick/target/release/cardbrick-cli deck list

# Add to PATH (optional)
export PATH="$PATH:/full/path/to/CardBrick/target/release"
```

#### 2. "No cached decks found" or Empty deck list

**Problem:** CLI returns empty deck list

**Debug Steps:**
```bash
# Check cache directories exist
ls -la ./test_cache/
ls -la ./precache/
ls -la /storage/applications/CardBrick/decks/  # RG35XX
ls -la /mnt/SDCARD/cardbrick/decks/           # TrimUI

# Check specific deck structure
ls -la ./test_cache/7766cd891981a68c147e026fdef1520548f9fed81618c0b114749bd7eeacb0d6/
# Should show: manifest.json and deck_name.db

# Validate manifest.json
cat ./test_cache/*/manifest.json | python3 -m json.tool
```

**Solutions:**
- Run the main CardBrick app to cache decks first
- Use the precache scripts to prepare deck data
- Check file permissions on cache directories

#### 3. "Failed to read session file"

**Problem:** Session operations fail

**Debug Steps:**
```bash
# Check session directory
ls -la /tmp/cardbrick-sessions/

# Check specific session file
cat /tmp/cardbrick-sessions/your-session-id.json

# Check permissions
ls -la /tmp/cardbrick-sessions/
```

**Solutions:**
```bash
# Clean up stale sessions
rm -rf /tmp/cardbrick-sessions/*

# Fix permissions
chmod 755 /tmp/cardbrick-sessions/
chmod 644 /tmp/cardbrick-sessions/*.json
```

#### 4. "Database is locked" or SQL errors

**Problem:** SQLite database issues

**Debug Steps:**
```bash
# Check database files
ls -la ~/.cardbrick/progress.db
ls -la ./test_cache/*/deck_name.db

# Test database access
sqlite3 ~/.cardbrick/progress.db ".tables"
```

**Solutions:**
```bash
# Stop all CardBrick processes
pkill cardbrick

# Check for other processes using database
lsof ~/.cardbrick/progress.db

# Backup and recreate if corrupted
cp ~/.cardbrick/progress.db ~/.cardbrick/progress.db.backup
rm ~/.cardbrick/progress.db
# CLI will recreate on next use
```

#### 5. JSON parsing errors

**Problem:** Invalid JSON responses from CLI

**Debug Steps:**
```bash
# Run command and check output
./target/release/cardbrick-cli deck list | python3 -m json.tool

# Check for error messages mixed with JSON
./target/release/cardbrick-cli deck list 2>&1 | cat -A
```

**Solutions:**
- Check for stdout/stderr mixing in your application
- Ensure CLI is built with release optimizations
- Update to latest CLI version

#### 6. Performance issues

**Problem:** CLI commands are slow

**Debug Steps:**
```bash
# Time individual commands
time ./target/release/cardbrick-cli deck list
time ./target/release/cardbrick-cli progress daily

# Check system resources
top
df -h  # Check disk space
```

**Solutions:**
- Use release build instead of debug build
- Clean up old session files
- Check available disk space
- Reduce concurrent CLI calls

#### 7. Frontend integration issues

**Problem:** Frontend can't communicate with CLI

**JavaScript Debug:**
```javascript
// Add detailed logging
async function debugCLI(command) {
    console.log('Running CLI command:', command);

    try {
        const start = Date.now();
        const result = await runCLI(command);
        const duration = Date.now() - start;

        console.log('CLI Success:', {
            command,
            duration: `${duration}ms`,
            result
        });

        return result;
    } catch (error) {
        console.error('CLI Error:', {
            command,
            error: error.message,
            stack: error.stack
        });
        throw error;
    }
}
```

**Python Debug:**
```python
import logging

logging.basicConfig(level=logging.DEBUG)
logger = logging.getLogger(__name__)

def debug_cli(command):
    logger.debug(f"Running CLI: {command}")

    try:
        result = subprocess.run(
            command.split(),
            capture_output=True,
            text=True,
            check=True
        )

        logger.debug(f"CLI stdout: {result.stdout}")
        logger.debug(f"CLI stderr: {result.stderr}")

        return json.loads(result.stdout)
    except Exception as e:
        logger.error(f"CLI failed: {e}")
        raise
```

### Getting Help

#### 1. Enable Debug Logging
```bash
# Run with debug logging
RUST_LOG=debug ./target/release/cardbrick-cli deck list

# Or for more verbose output
RUST_LOG=trace ./target/release/cardbrick-cli deck list
```

#### 2. Check CLI Version and Build Info
```bash
./target/release/cardbrick-cli --version
```

#### 3. Generate Diagnostic Report
```bash
#!/bin/bash
# diagnostic_report.sh

echo "CardBrick CLI Diagnostic Report"
echo "==============================="
echo "Date: $(date)"
echo ""

echo "System Information:"
echo "  OS: $(uname -a)"
echo "  User: $(whoami)"
echo "  PWD: $(pwd)"
echo ""

echo "CLI Binary:"
echo "  Path: ./target/release/cardbrick-cli"
echo "  Exists: $(test -f ./target/release/cardbrick-cli && echo 'Yes' || echo 'No')"
echo "  Executable: $(test -x ./target/release/cardbrick-cli && echo 'Yes' || echo 'No')"
echo "  Size: $(ls -lh ./target/release/cardbrick-cli 2>/dev/null | awk '{print $5}' || echo 'N/A')"
echo ""

echo "Cache Directories:"
echo "  ./test_cache: $(test -d ./test_cache && echo 'Exists' || echo 'Missing')"
echo "  ./precache: $(test -d ./precache && echo 'Exists' || echo 'Missing')"
echo "  Contents: $(ls -1 ./test_cache/ 2>/dev/null | wc -l || echo '0') directories"
echo ""

echo "Progress Database:"
echo "  Path: ~/.cardbrick/progress.db"
echo "  Exists: $(test -f ~/.cardbrick/progress.db && echo 'Yes' || echo 'No')"
echo "  Size: $(ls -lh ~/.cardbrick/progress.db 2>/dev/null | awk '{print $5}' || echo 'N/A')"
echo ""

echo "Session Directory:"
echo "  Path: /tmp/cardbrick-sessions"
echo "  Exists: $(test -d /tmp/cardbrick-sessions && echo 'Yes' || echo 'No')"
echo "  Sessions: $(ls -1 /tmp/cardbrick-sessions/ 2>/dev/null | wc -l || echo '0') files"
echo ""

echo "CLI Test:"
if ./target/release/cardbrick-cli --help > /dev/null 2>&1; then
    echo "  Basic functionality: OK"
else
    echo "  Basic functionality: FAILED"
fi

if output=$(./target/release/cardbrick-cli deck list 2>&1); then
    if echo "$output" | python3 -m json.tool > /dev/null 2>&1; then
        echo "  Deck list: OK"
    else
        echo "  Deck list: Invalid JSON"
    fi
else
    echo "  Deck list: FAILED"
fi
```

#### 4. Community Support
- Check the main CardBrick documentation
- Search for similar issues in the project repository
- When reporting issues, include:
  - CLI diagnostic report (above)
  - Exact command that failed
  - Complete error output
  - Operating system and version
  - CardBrick version

---

This guide provides everything a junior developer needs to successfully integrate CardBrick CLI into any frontend application. The examples are comprehensive and the error handling approaches will help build robust applications that provide a good user experience even when things go wrong.