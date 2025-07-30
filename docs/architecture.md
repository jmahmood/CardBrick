# CardBrick Sprint 4 Architecture

## Overview

Sprint 4 implements a "doorbell-pull" sync pipeline enabling secure deck synchronization between desktop and handheld devices.

## System Components

```
┌─────────────────────────────────────────────────────────────────┐
│                        Desktop Environment                       │
├─────────────────────────────────────────────────────────────────┤
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐ │
│  │  CardBrick CLI  │  │  Sync Daemon    │  │   Web GUI       │ │
│  │                 │  │                 │  │  (Future)       │ │
│  │ • CSV Import    │  │ • HTTP Server   │  │ • React App     │ │
│  │ • MD Import     │  │ • mDNS Service  │  │ • Token Auth    │ │
│  │ • Queue Mgmt    │  │ • Crypto Auth   │  │ • Import UI     │ │
│  └─────────────────┘  └─────────────────┘  └─────────────────┘ │
│           │                      │                      │       │
│           └──────────────────────┼──────────────────────┘       │
│                                  │                              │
│  ┌─────────────────┐             │                              │
│  │ Unix Socket     │◄────────────┘                              │
│  │ Auth Server     │                                            │
│  └─────────────────┘                                            │
└─────────────────────────────────────────────────────────────────┘
                                   │
                                   │ HTTP/mDNS
                        ┌──────────┼──────────┐
                        │      Network        │
                        └──────────┼──────────┘
                                   │
┌─────────────────────────────────────────────────────────────────┐
│                      Handheld Device                            │
├─────────────────────────────────────────────────────────────────┤
│  ┌─────────────────┐             ┌─────────────────┐            │
│  │   CardBrick     │             │   sync_ring     │            │
│  │   Main App      │             │                 │            │
│  │                 │             │ • Service       │            │
│  │ • Study Mode    │◄────────────┤   Discovery     │            │
│  │ • Progress      │             │ • Crypto Sign   │            │
│  │ • Sync Button   │             │ • HTTP Client   │            │
│  └─────────────────┘             └─────────────────┘            │
│                                           │                     │
│  ┌─────────────────────────────────────────┼──────────────────┐ │
│  │              File System                │                  │ │
│  │  /flash/decks/                          │                  │ │
│  │  ├── deck1.cbdeck                       │                  │ │
│  │  ├── deck2.cbdeck                       │                  │ │
│  │  └── ...                                │                  │ │
│  └─────────────────────────────────────────┼──────────────────┘ │
│                                            │                    │
│                                   ┌────────▼────────┐           │
│                                   │  SSH/rsync      │           │
│                                   │  (over network) │           │
│                                   └─────────────────┘           │
└─────────────────────────────────────────────────────────────────┘
```

## Data Flow

### 1. Deck Import (Desktop)

```
User → cardbrick-cli → Importer → .cbdeck file → Sync Queue
```

1. User imports CSV/Markdown via CLI
2. Importer converts to CardBrick format
3. Deck stored in `/var/lib/cardbrick/inbox/`
4. Optional: Added to sync queue with "needs-sync" flag

### 2. Service Discovery

```
Desktop: Avahi mDNS → _cardbrick._tcp (port 6429)
Handheld: Service Discovery → HTTP probe → Found desktop
```

1. Desktop advertises sync service via mDNS
2. Handheld scans network for CardBrick services
3. Validates service availability via `/health` endpoint

### 3. Authentication Flow

```
Handheld → Doorbell Request (signed) → Desktop → Validation → Response
```

**Request Structure:**
```json
{
  "device_ip": "192.168.1.50",
  "ssh_port": 22,
  "pubkey_fingerprint": "sha256:abc123...",
  "ready_until": 1640995200,
  "signature": "ed25519_signature_hex"
}
```

**Validation Steps:**
1. Check timestamp (within 5 minutes)
2. Find device key by fingerprint
3. Verify ed25519 signature
4. Check replay protection
5. Rate limiting (15-minute window)

### 4. Sync Operation

```
Desktop → rsync over SSH → /flash/decks/ ← Handheld
```

**rsync Command:**
```bash
rsync -e "ssh -i /var/lib/cardbrick/id_ed25519 -o StrictHostKeyChecking=yes -p 22" \
      --archive --verbose --compress --stats --partial \
      cardbrick@192.168.1.50:/flash/decks/ /var/lib/cardbrick/inbox/
```

**Security Features:**
- SSH key-based authentication
- Forced command wrapper (rsync only)
- StrictHostKeyChecking enabled
- User privilege separation

### 5. Status Reporting

```
Desktop → POST /sync_result → Handheld (optional)
Desktop → Update job status → Local storage
```

## Security Architecture

### Cryptographic Design

**Key Management:**
- Desktop: ed25519 daemon key pair in `/var/lib/cardbrick/`
- Handheld: Device-specific key pair in `/etc/cardbrick/`
- SSH: Separate ed25519 keys for file transfer

**Signature Verification:**
```python
# Canonical message format
message = {
    "device_ip": "...",
    "ssh_port": 22,
    "pubkey_fingerprint": "...",
    "ready_until": timestamp
}
canonical_json = json.dumps(message, sort_keys=True, separators=(',', ':'))
signature = ed25519_sign(private_key, canonical_json.encode())
```

**Replay Protection:**
- `ready_until` timestamp (5-minute window)
- Signature hash tracking (prevents reuse)
- Rate limiting per device IP

### System Hardening

**systemd Security Features:**
```ini
DynamicUser=yes
ProtectSystem=strict
ProtectHome=yes
RestrictAddressFamilies=AF_INET AF_INET6 AF_UNIX
CapabilityBoundingSet=
NoNewPrivileges=yes
SystemCallFilter=@system-service
MemoryDenyWriteExecute=yes
```

**Network Restrictions:**
- IP allowlists (private networks only)
- Port restrictions (6429 for HTTP, 22 for SSH)
- Avahi service isolation

## API Specification

### OpenAPI 3.1 Endpoints

**Core Endpoints:**
- `GET /health` - Service health check
- `GET /status` - Detailed daemon status  
- `POST /doorbell` - Handheld sync requests
- `GET /docs` - Interactive API documentation

**Management Endpoints:**
- `POST /import` - Import deck files (future GUI)
- `POST /sync` - Manual sync trigger (future)
- `GET /auth/stats` - Local authentication statistics

**Authentication:**
- Unix socket for local access (`/run/user/*/cardbrick/auth.sock`)
- Single-use tokens with 5-minute TTL
- No authentication required for doorbell (cryptographically signed)

### Data Models

**Doorbell Request:**
```typescript
interface DoorbellRequest {
  device_ip: string;
  ssh_port: number;
  pubkey_fingerprint: string;
  ready_until: number;  // Unix timestamp
  signature: string;    // hex-encoded ed25519
}
```

**Sync Result:**
```typescript
interface SyncResult {
  success: boolean;
  files_transferred: number;
  bytes_transferred: number;
  duration_seconds: number;
  error_message?: string;
}
```

## Performance Characteristics

### Resource Usage
- **Desktop Daemon**: ~10MB RAM, minimal CPU when idle
- **Handheld Client**: ~1MB binary, <5MB RAM during sync
- **Network**: HTTP keepalive, rsync compression

### Scalability
- **Concurrent Syncs**: Limited by rsync processes (typically 1-3)
- **Device Limit**: No artificial limit (tested with 10+ devices)
- **File Size**: rsync handles large files efficiently with `--partial`

### Rate Limiting
- **Doorbell Requests**: 1 per device per 15 minutes
- **Token Generation**: 100/hour per Unix socket client
- **Health Checks**: Unlimited (for monitoring)

## File Formats

### .cbdeck Format
```json
{
  "metadata": {
    "name": "Japanese Vocabulary",
    "description": "JLPT N5 vocabulary cards", 
    "version": "1.0",
    "created_at": "2024-01-15T10:30:00Z",
    "card_count": 150
  },
  "cards": [
    {
      "front": "こんにちは",
      "back": "Hello (polite)",
      "tags": ["greeting", "polite"],
      "media": []
    }
  ]
}
```

### Import Support
- **CSV**: `front,back[,tags]` format
- **Markdown**: Header-based Q&A, Anki-style separators
- **Future**: Anki .apkg, Memrise, Quizlet exports

## Deployment Architecture

### Development Environment
```bash
# Desktop development
python -m cardbrick_syncd.main
cardbrick-cli import test.csv

# Handheld testing  
cargo run --bin sync_ring
```

### Production Deployment
```bash
# Desktop production
systemctl start cardbrick-syncd.service
curl http://localhost:6429/health

# Handheld production
/usr/local/bin/sync_ring  # Triggered by UI or cron
```

### CI/CD Pipeline
- **GitHub Actions**: Rust + Python testing
- **Docker Compose**: Integration testing with real network
- **Security Scanning**: cargo-audit, safety, bandit
- **Cross-compilation**: ARM64 binaries for handheld devices

## Future Extensions

### Sprint 5+ Features
- **GUI Client**: React-based desktop management interface
- **Mobile Apps**: iOS/Android companion apps
- **Cloud Sync**: Optional cloud backup with E2E encryption
- **Multi-User**: Shared deck repositories and collaboration
- **Analytics**: Study progress tracking and insights

### Protocol Extensions
- **Incremental Sync**: Delta updates for large decks
- **Bidirectional Sync**: Progress data from handheld to desktop
- **Mesh Networking**: Device-to-device sync without desktop
- **Real-time Updates**: WebSocket-based live sync

---

This architecture provides a secure, scalable foundation for CardBrick's sync capabilities while maintaining simplicity and reliability for the target handheld gaming device use case.