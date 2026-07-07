"""
Avahi service discovery for CardBrick sync daemon

Advertises _cardbrick._tcp service on port 6429 with TXT records:
- proto=http
- ver=1
"""

import logging
import socket
from typing import Optional

try:
    from .config import Config
except ImportError:
    from config import Config  # type: ignore[no-redef]

logger = logging.getLogger(__name__)

try:
    import avahi
    import dbus
    from dbus.mainloop.glib import DBusGMainLoop
    AVAHI_AVAILABLE = True
except ImportError as e:
    AVAHI_AVAILABLE = False
    logger.warning(f"Avahi not available: {e}")
    logger.info("mDNS service discovery will be disabled")

class AvahiService:
    """Manages Avahi service advertisement for CardBrick sync"""
    
    def __init__(self, config: Config):
        self.config = config
        self.group: Optional[object] = None
        self.server: Optional[object] = None
        
        if not AVAHI_AVAILABLE:
            logger.warning("Avahi not available, service discovery disabled")
            return
            
        # Initialize D-Bus
        DBusGMainLoop(set_as_default=True)
        self.bus = dbus.SystemBus()
        
    def start(self):
        """Start advertising the CardBrick service"""
        if not AVAHI_AVAILABLE:
            logger.info("Avahi not available, skipping service advertisement")
            return
            
        try:
            # Get Avahi server
            server_obj = self.bus.get_object(avahi.DBUS_NAME, avahi.DBUS_PATH_SERVER)
            self.server = dbus.Interface(server_obj, avahi.DBUS_INTERFACE_SERVER)
            
            # Create entry group
            group_obj = self.bus.get_object(avahi.DBUS_NAME, self.server.EntryGroupNew())
            self.group = dbus.Interface(group_obj, avahi.DBUS_INTERFACE_ENTRY_GROUP)
            
            # Get local hostname
            hostname = socket.gethostname()
            
            # TXT records for service discovery
            txt_records = [
                b"proto=http",
                b"ver=1"
            ]
            
            # Add service to entry group
            self.group.AddService(
                avahi.IF_UNSPEC,           # interface
                avahi.PROTO_UNSPEC,       # protocol  
                dbus.UInt32(0),           # flags
                f"CardBrick Sync on {hostname}",  # service name
                self.config.service_name,  # service type
                "",                       # domain (empty = default)
                "",                       # host (empty = localhost)
                dbus.UInt16(self.config.http_port),  # port
                txt_records               # TXT records
            )
            
            # Commit the service
            self.group.Commit()
            
            logger.info(f"Advertised {self.config.service_name} on port {self.config.http_port}")
            
        except Exception as e:
            logger.error(f"Failed to start Avahi service: {e}")
            
    def stop(self):
        """Stop advertising the service"""
        if not AVAHI_AVAILABLE:
            return
            
        try:
            if self.group:
                self.group.Reset()
                logger.info("Stopped Avahi service advertisement")
        except Exception as e:
            logger.error(f"Error stopping Avahi service: {e}")
            
    def __del__(self):
        """Cleanup on destruction"""
        self.stop()

class SimpleDiscovery:
    """Fallback discovery service when Avahi is not available"""
    
    def __init__(self, config: Config):
        self.config = config
        self.running = False
        
    def start(self):
        """Start simple discovery (no-op, relies on direct IP connection)"""
        if not AVAHI_AVAILABLE:
            logger.info("Using simplified discovery - devices can connect directly to this machine's IP")
            logger.info(f"CardBrick daemon available at: http://{self._get_local_ip()}:{self.config.http_port}")
        self.running = True
        
    def stop(self):
        """Stop discovery"""
        self.running = False
        
    def _get_local_ip(self):
        """Get local IP address for display"""
        try:
            # Connect to a remote address to determine local IP
            import socket
            s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
            s.connect(("8.8.8.8", 80))
            local_ip = s.getsockname()[0]
            s.close()
            return local_ip
        except Exception:
            return "localhost"
