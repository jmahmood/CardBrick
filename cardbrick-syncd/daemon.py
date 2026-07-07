"""
Core CardBrick sync daemon implementation

Handles:
- FastAPI HTTP server with /doorbell and /health endpoints
- Doorbell request validation and authentication
- Rsync operations for deck synchronization
- Rate limiting and replay protection
"""

import asyncio
import logging
import time
from datetime import datetime, timezone
from typing import Dict, Optional, Any

from fastapi import FastAPI
from pydantic import BaseModel, Field

try:
    from .config import Config
    from .crypto import CryptoManager, ValidationError
    from .sync import SyncManager
    from .auth import LocalAuthManager
except ImportError:
    from config import Config  # type: ignore[no-redef]
    from crypto import CryptoManager, ValidationError  # type: ignore[no-redef]
    from sync import SyncManager  # type: ignore[no-redef]
    from auth import LocalAuthManager  # type: ignore[no-redef]

logger = logging.getLogger(__name__)

class DoorbellRequest(BaseModel):
    """Schema for doorbell sync requests from handheld devices"""
    device_ip: str = Field(..., description="IP address of the handheld device")
    ssh_port: int = Field(22, description="SSH port on the handheld device")
    pubkey_fingerprint: str = Field(..., description="Public key fingerprint for device identification")
    ready_until: int = Field(..., description="Unix timestamp when request expires")
    signature: str = Field(..., description="ed25519 signature of the request")
    backup_mode: bool = Field(False, description="Whether this is a manual backup request")

class DoorbellResponse(BaseModel):
    """Response to doorbell requests"""
    status: str = Field(..., description="Status: accepted, rejected, or error")
    message: str = Field("", description="Human-readable status message")
    job_id: Optional[str] = Field(None, description="Unique job ID for tracking")

class HealthResponse(BaseModel):
    """Health check response"""
    status: str = Field("healthy", description="Service health status")
    version: str = Field("0.1.0", description="Daemon version")
    uptime_seconds: int = Field(..., description="Daemon uptime in seconds")
    active_jobs: int = Field(0, description="Number of active sync jobs")

class StatusResponse(BaseModel):
    """Detailed status response"""
    daemon: Dict[str, Any] = Field(..., description="Daemon information")
    jobs: Dict[str, Any] = Field(..., description="Job information")

class ImportRequest(BaseModel):
    """Request to import a deck file"""
    file_path: str = Field(..., description="Path to the file to import")
    deck_name: Optional[str] = Field(None, description="Custom name for the deck")
    auto_sync: bool = Field(False, description="Add to sync queue automatically")

class ImportResponse(BaseModel):
    """Response from import operation"""
    status: str = Field(..., description="Import status")
    message: str = Field(..., description="Status message")
    deck_path: Optional[str] = Field(None, description="Path to created deck file")

class SyncTriggerResponse(BaseModel):
    """Response from manual sync trigger"""
    status: str = Field(..., description="Sync trigger status")
    message: str = Field(..., description="Status message")
    triggered_jobs: int = Field(0, description="Number of sync jobs triggered")

class AuthStatsResponse(BaseModel):
    """Authentication statistics response"""
    auth_enabled: bool = Field(..., description="Whether local auth is enabled")
    socket_path: str = Field(..., description="Path to auth Unix socket")
    token_stats: Dict[str, Any] = Field(..., description="Token statistics")

class SyncDaemon:
    """Main sync daemon class"""
    
    def __init__(self, config: Config):
        self.config = config
        self.start_time = time.time()
        self.active_jobs: Dict[str, Dict[str, Any]] = {}
        self.rate_limit_tracker: Dict[str, float] = {}
        
        # Initialize managers
        self.crypto = CryptoManager(config)
        self.sync_manager = SyncManager(config)
        self.auth_manager = LocalAuthManager()
        
        # Initialize FastAPI app
        self.app = FastAPI(
            title="CardBrick Sync Daemon",
            description="Desktop sync daemon for CardBrick handheld devices",
            version="0.1.0",
            docs_url="/docs",
            redoc_url="/redoc",
            openapi_url="/openapi.json",
            contact={
                "name": "CardBrick Team",
                "url": "https://github.com/CardBrick/CardBrick"
            },
            license_info={
                "name": "GPL-3.0",
                "url": "https://www.gnu.org/licenses/gpl-3.0.html"
            }
        )
        
        # Register endpoints
        self._register_endpoints()
        
    def _register_endpoints(self):
        """Register FastAPI endpoints"""
        
        @self.app.post("/doorbell", response_model=DoorbellResponse)
        async def doorbell(request: DoorbellRequest) -> DoorbellResponse:
            """Handle doorbell sync requests from handheld devices"""
            return await self._handle_doorbell(request)
            
        @self.app.get("/health", response_model=HealthResponse)
        async def health() -> HealthResponse:
            """Health check endpoint"""
            return HealthResponse(
                status="healthy",
                version="0.1.0",
                uptime_seconds=int(time.time() - self.start_time),
                active_jobs=len(self.active_jobs)
            )
            
        @self.app.get("/status", response_model=StatusResponse)
        async def status() -> StatusResponse:
            """Detailed status information"""
            return StatusResponse(
                daemon={
                    "version": "0.1.0",
                    "uptime_seconds": int(time.time() - self.start_time),
                    "config": {
                        "http_port": self.config.http_port,
                        "data_dir": str(self.config.data_dir),
                        "inbox_dir": str(self.config.inbox_dir)
                    }
                },
                jobs={
                    "active": len(self.active_jobs),
                    "details": list(self.active_jobs.values())
                }
            )
            
        @self.app.post("/import", response_model=ImportResponse)
        async def import_deck(request: ImportRequest) -> ImportResponse:
            """Import a deck file (for future GUI integration)"""
            # This endpoint would be used by future GUI clients
            # For now, return not implemented
            return ImportResponse(
                status="not_implemented", 
                message="Use cardbrick-cli for imports",
                deck_path=None
            )
            
        @self.app.post("/sync", response_model=SyncTriggerResponse)
        async def manual_sync() -> SyncTriggerResponse:
            """Manually trigger sync operations (desktop only)"""
            # This could trigger sync of queued decks to known devices
            return SyncTriggerResponse(
                status="not_implemented", 
                message="Manual sync not yet implemented",
                triggered_jobs=0
            )
            
        @self.app.get("/auth/stats", response_model=AuthStatsResponse)
        async def auth_stats() -> AuthStatsResponse:
            """Get local authentication statistics"""
            return AuthStatsResponse(
                auth_enabled=True,
                socket_path=str(self.auth_manager.socket_path),
                token_stats=self.auth_manager.get_token_stats()
            )
    
    async def _handle_doorbell(self, request: DoorbellRequest) -> DoorbellResponse:
        """Process a doorbell request with validation and rate limiting"""
        try:
            # Rate limiting check
            if self._is_rate_limited(request.device_ip):
                logger.warning(f"Rate limited doorbell from {request.device_ip}")
                return DoorbellResponse(
                    status="rejected",
                    message="Rate limited - max 1 request per 15 minutes",
                    job_id=None
                )
            
            # Validate request signature and timing
            await self.crypto.validate_doorbell(request)
            
            # Create sync job
            job_id = f"sync_{int(time.time())}_{request.device_ip.replace('.', '_')}"
            
            # Store job info
            self.active_jobs[job_id] = {
                "id": job_id,
                "device_ip": request.device_ip,
                "ssh_port": request.ssh_port,
                "pubkey_fingerprint": request.pubkey_fingerprint,
                "created_at": datetime.now(timezone.utc).isoformat(),
                "status": "accepted"
            }
            
            # Update rate limiting
            self.rate_limit_tracker[request.device_ip] = time.time()
            
            # Start sync operation asynchronously  
            asyncio.create_task(self._perform_sync(job_id, request))
            
            logger.info(f"Accepted doorbell from {request.device_ip}, job {job_id}")
            
            return DoorbellResponse(
                status="accepted",
                message="Sync job started",
                job_id=job_id
            )
            
        except ValidationError as e:
            logger.warning(f"Invalid doorbell from {request.device_ip}: {e}")
            return DoorbellResponse(
                status="rejected", 
                message=str(e),
                job_id=None
            )
        except Exception as e:
            logger.error(f"Error handling doorbell from {request.device_ip}: {e}")
            return DoorbellResponse(
                status="error",
                message="Internal server error",
                job_id=None
            )
    
    def _is_rate_limited(self, device_ip: str) -> bool:
        """Check if device is rate limited"""
        if device_ip not in self.rate_limit_tracker:
            return False
            
        last_request = self.rate_limit_tracker[device_ip]
        return (time.time() - last_request) < self.config.rate_limit_window_seconds
    
    async def _perform_sync(self, job_id: str, request: DoorbellRequest):
        """Perform the actual sync operation"""
        if job_id not in self.active_jobs:
            return
            
        job = self.active_jobs[job_id]
        
        try:
            job["status"] = "syncing"
            job["started_at"] = datetime.now(timezone.utc).isoformat()
            
            # Perform rsync operation
            result = await self.sync_manager.sync_from_device(
                device_ip=request.device_ip,
                ssh_port=request.ssh_port,
                pubkey_fingerprint=request.pubkey_fingerprint
            )
            
            job["status"] = "completed" if result.success else "failed"
            job["completed_at"] = datetime.now(timezone.utc).isoformat()
            job["result"] = result.to_dict()
            
            # Notify device of completion (optional)
            await self._notify_device_completion(request.device_ip, job_id, result)
            
            logger.info(f"Sync job {job_id} completed with status: {job['status']}")
            
        except Exception as e:
            job["status"] = "error"
            job["error"] = str(e)
            job["completed_at"] = datetime.now(timezone.utc).isoformat()
            logger.error(f"Sync job {job_id} failed: {e}")
        finally:
            # Clean up job after some time (keep for status queries)
            await asyncio.sleep(300)  # Keep for 5 minutes
            if job_id in self.active_jobs:
                del self.active_jobs[job_id]
    
    async def _notify_device_completion(self, device_ip: str, job_id: str, result):
        """Optional: notify device of sync completion"""
        # This could POST to device's /sync_result endpoint
        # For now, we'll log it
        logger.info(f"Sync completed for {device_ip}, job {job_id}: {result.success}")
    
    async def start(self):
        """Initialize daemon components"""
        await self.crypto.initialize()
        await self.auth_manager.start()
        logger.info("CardBrick sync daemon initialized")
        
    async def shutdown(self):
        """Graceful shutdown"""
        logger.info("Shutting down sync daemon...")
        # Cancel any running sync jobs
        for job_id in list(self.active_jobs.keys()):
            job = self.active_jobs[job_id]
            if job["status"] in ["accepted", "syncing"]:
                job["status"] = "cancelled"
                job["completed_at"] = datetime.now(timezone.utc).isoformat()
        # Stop auth manager
        await self.auth_manager.stop()
        logger.info("Sync daemon shutdown complete")
