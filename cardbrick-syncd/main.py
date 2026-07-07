#!/usr/bin/env python3
"""
CardBrick Desktop Sync Daemon

Main entry point for the desktop sync daemon that:
1. Advertises _cardbrick._tcp service via Avahi
2. Handles doorbell requests from handheld devices  
3. Performs authenticated rsync operations
4. Provides status and health endpoints
"""

import asyncio
import logging
import signal
import sys
from typing import Optional

import uvicorn

try:
    # Try relative imports first (when run as part of package)
    from .daemon import SyncDaemon
    from .discovery import AvahiService, SimpleDiscovery, AVAHI_AVAILABLE
    from .config import Config
except ImportError:
    # Fall back to absolute imports (when run as main module)
    from daemon import SyncDaemon  # type: ignore[no-redef]
    from discovery import AvahiService, SimpleDiscovery, AVAHI_AVAILABLE  # type: ignore[no-redef]
    from config import Config  # type: ignore[no-redef]

# Configure logging
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger(__name__)

# Global daemon instance for graceful shutdown
daemon_instance: Optional[SyncDaemon] = None
discovery_service = None

def signal_handler(signum, frame):
    """Handle shutdown signals gracefully"""
    logger.info(f"Received signal {signum}, shutting down...")
    if daemon_instance:
        asyncio.create_task(daemon_instance.shutdown())
    if discovery_service:
        discovery_service.stop()
    sys.exit(0)

async def main():
    """Main daemon entry point"""
    global daemon_instance, discovery_service
    
    # Load configuration
    config = Config.load()
    
    # Initialize daemon
    daemon_instance = SyncDaemon(config)
    
    # Initialize service discovery (Avahi or fallback)
    if AVAHI_AVAILABLE:
        discovery_service = AvahiService(config)
        logger.info("Using Avahi for mDNS service discovery")
    else:
        discovery_service = SimpleDiscovery(config)
        logger.info("Using simplified discovery (Avahi not available)")
    
    # Set up signal handlers
    signal.signal(signal.SIGTERM, signal_handler)
    signal.signal(signal.SIGINT, signal_handler)
    
    try:
        # Start service discovery
        logger.info("Starting service discovery...")
        discovery_service.start()
        
        # Start daemon
        logger.info("Starting CardBrick sync daemon...")
        await daemon_instance.start()
        
        # Start FastAPI server
        config_uvicorn = uvicorn.Config(
            daemon_instance.app,
            host="0.0.0.0",
            port=config.http_port,
            log_level="info"
        )
        server = uvicorn.Server(config_uvicorn)
        
        logger.info(f"Starting HTTP server on port {config.http_port}")
        await server.serve()
        
    except Exception as e:
        logger.error(f"Daemon failed: {e}")
        sys.exit(1)
    finally:
        if daemon_instance:
            await daemon_instance.shutdown()
        if discovery_service:
            discovery_service.stop()

if __name__ == "__main__":
    asyncio.run(main())
