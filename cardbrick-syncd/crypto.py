"""
Cryptographic operations for CardBrick sync daemon

Handles:
- ed25519 key generation and management
- Doorbell request signature verification
- Replay attack prevention
- Device authentication
"""

import hashlib
import json
import logging
import time
from typing import Dict, Set, Optional

from cryptography.hazmat.primitives import serialization
from cryptography.hazmat.primitives.asymmetric import ed25519
from cryptography.exceptions import InvalidSignature

try:
    from .config import Config
except ImportError:
    from config import Config

logger = logging.getLogger(__name__)

class ValidationError(Exception):
    """Raised when doorbell validation fails"""
    pass

class CryptoManager:
    """Manages cryptographic operations for sync daemon"""
    
    def __init__(self, config: Config):
        self.config = config
        self.daemon_private_key: Optional[ed25519.Ed25519PrivateKey] = None
        self.daemon_public_key: Optional[ed25519.Ed25519PublicKey] = None
        self.device_public_keys: Dict[str, ed25519.Ed25519PublicKey] = {}
        self.processed_signatures: Set[str] = set()  # Replay protection
        
    async def initialize(self):
        """Initialize cryptographic components"""
        await self._load_or_generate_daemon_keys()
        await self._load_device_public_keys()
        logger.info("Crypto manager initialized")
        
    async def _load_or_generate_daemon_keys(self):
        """Load existing daemon keys or generate new ones"""
        private_key_path = self.config.keys_dir / "daemon_private.pem"
        public_key_path = self.config.keys_dir / "daemon_public.pem"
        
        if private_key_path.exists() and public_key_path.exists():
            # Load existing keys
            try:
                with open(private_key_path, 'rb') as f:
                    private_pem = f.read()
                    self.daemon_private_key = serialization.load_pem_private_key(
                        private_pem, 
                        password=None
                    )
                    
                with open(public_key_path, 'rb') as f:
                    public_pem = f.read()
                    self.daemon_public_key = serialization.load_pem_public_key(public_pem)
                    
                logger.info("Loaded existing daemon keys")
                return
            except Exception as e:
                logger.warning(f"Failed to load daemon keys, generating new ones: {e}")
        
        # Generate new keys
        self.daemon_private_key = ed25519.Ed25519PrivateKey.generate()
        self.daemon_public_key = self.daemon_private_key.public_key()
        
        # Save keys
        private_pem = self.daemon_private_key.private_bytes(
            encoding=serialization.Encoding.PEM,
            format=serialization.PrivateFormat.PKCS8,
            encryption_algorithm=serialization.NoEncryption()
        )
        
        public_pem = self.daemon_public_key.public_bytes(
            encoding=serialization.Encoding.PEM,
            format=serialization.PublicFormat.SubjectPublicKeyInfo
        )
        
        # Write with secure permissions
        private_key_path.write_bytes(private_pem)
        private_key_path.chmod(0o600)
        
        public_key_path.write_bytes(public_pem)
        public_key_path.chmod(0o644)
        
        logger.info("Generated new daemon keys")
        
    async def _load_device_public_keys(self):
        """Load public keys for known devices"""
        devices_dir = self.config.keys_dir / "devices"
        devices_dir.mkdir(exist_ok=True)
        
        for key_file in devices_dir.glob("*.pem"):
            try:
                device_id = key_file.stem
                with open(key_file, 'rb') as f:
                    public_pem = f.read()
                    public_key = serialization.load_pem_public_key(public_pem)
                    self.device_public_keys[device_id] = public_key
                logger.info(f"Loaded public key for device: {device_id}")
            except Exception as e:
                logger.warning(f"Failed to load device key {key_file}: {e}")
                
    async def validate_doorbell(self, request) -> None:
        """
        Validate a doorbell request
        
        Raises ValidationError if validation fails
        """
        # Check timestamp
        current_time = int(time.time())
        if request.ready_until <= current_time:
            raise ValidationError("Request expired")
            
        if request.ready_until > current_time + self.config.max_doorbell_age_seconds:
            raise ValidationError("Request timestamp too far in future")
        
        # Find device public key by fingerprint
        device_public_key = None
        for device_id, pub_key in self.device_public_keys.items():
            if self._get_key_fingerprint(pub_key) == request.pubkey_fingerprint:
                device_public_key = pub_key
                break
                
        if not device_public_key:
            raise ValidationError(f"Unknown device fingerprint: {request.pubkey_fingerprint}")
        
        # Create canonical message for signature verification
        message_data = {
            "device_ip": request.device_ip,
            "ssh_port": request.ssh_port,
            "pubkey_fingerprint": request.pubkey_fingerprint,
            "ready_until": request.ready_until
        }
        
        # Sort keys for consistent serialization
        canonical_message = json.dumps(message_data, sort_keys=True, separators=(',', ':'))
        message_bytes = canonical_message.encode('utf-8')
        
        # Verify signature
        try:
            signature_bytes = bytes.fromhex(request.signature)
            device_public_key.verify(signature_bytes, message_bytes)
        except (ValueError, InvalidSignature) as e:
            raise ValidationError(f"Invalid signature: {e}")
        
        # Replay protection
        signature_hash = hashlib.sha256(signature_bytes).hexdigest()
        if signature_hash in self.processed_signatures:
            raise ValidationError("Signature already processed (replay attack)")
        
        # Store signature hash for replay protection
        self.processed_signatures.add(signature_hash)
        
        # Clean old signatures periodically (keep last 1000)
        if len(self.processed_signatures) > 1000:
            # Remove oldest half
            old_signatures = list(self.processed_signatures)[:500]
            for sig in old_signatures:
                self.processed_signatures.discard(sig)
                
        logger.info(f"Validated doorbell from device {request.pubkey_fingerprint}")
        
    def _get_key_fingerprint(self, public_key: ed25519.Ed25519PublicKey) -> str:
        """Get SHA256 fingerprint of a public key"""
        public_bytes = public_key.public_bytes(
            encoding=serialization.Encoding.Raw,
            format=serialization.PublicFormat.Raw
        )
        return hashlib.sha256(public_bytes).hexdigest()
        
    async def add_device_key(self, device_id: str, public_key_pem: bytes) -> str:
        """
        Add a new device public key
        
        Returns the key fingerprint
        """
        try:
            public_key = serialization.load_pem_public_key(public_key_pem)
            if not isinstance(public_key, ed25519.Ed25519PublicKey):
                raise ValueError("Not an ed25519 public key")
                
            fingerprint = self._get_key_fingerprint(public_key)
            
            # Save to file
            device_key_path = self.config.keys_dir / "devices" / f"{device_id}.pem"
            device_key_path.write_bytes(public_key_pem)
            device_key_path.chmod(0o644)
            
            # Add to memory
            self.device_public_keys[device_id] = public_key
            
            logger.info(f"Added device key for {device_id}, fingerprint: {fingerprint}")
            return fingerprint
            
        except Exception as e:
            raise ValidationError(f"Invalid device public key: {e}")
            
    def get_daemon_public_key_pem(self) -> bytes:
        """Get daemon's public key in PEM format"""
        if not self.daemon_public_key:
            raise RuntimeError("Daemon not initialized")
            
        return self.daemon_public_key.public_bytes(
            encoding=serialization.Encoding.PEM,
            format=serialization.PublicFormat.SubjectPublicKeyInfo
        )