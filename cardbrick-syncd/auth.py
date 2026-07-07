"""
Local authentication for CardBrick sync daemon

Provides Unix socket-based authentication for local GUI clients.
Issues single-use tokens with 5-minute TTL.
"""

import asyncio
import json
import logging
import os
import secrets
import time
from pathlib import Path
from typing import Dict, Optional, Set
from dataclasses import dataclass, asdict

logger = logging.getLogger(__name__)

@dataclass
class AuthToken:
    """Represents an authentication token"""
    token: str
    issued_at: float
    expires_at: float
    client_pid: Optional[int] = None
    used: bool = False
    
    def is_valid(self) -> bool:
        """Check if token is still valid"""
        return not self.used and time.time() < self.expires_at

    def to_dict(self) -> Dict:
        """Convert to dictionary"""
        return asdict(self)

class LocalAuthManager:
    """Manages local authentication via Unix sockets"""
    
    def __init__(self, runtime_dir: Optional[Path] = None, token_ttl: int = 300):
        self.token_ttl = token_ttl  # 5 minutes default
        self.tokens: Dict[str, AuthToken] = {}
        self.used_tokens: Set[str] = set()  # Track used tokens
        
        # Determine runtime directory
        if runtime_dir:
            self.runtime_dir = Path(runtime_dir)
        else:
            # Use XDG_RUNTIME_DIR or fallback
            xdg_runtime = os.getenv('XDG_RUNTIME_DIR')
            if xdg_runtime:
                self.runtime_dir = Path(xdg_runtime) / 'cardbrick'
            else:
                self.runtime_dir = Path('/tmp/cardbrick-auth')
        
        self.socket_path = self.runtime_dir / 'auth.sock'
        self.server = None
        
        # Ensure runtime directory exists with proper permissions
        self.runtime_dir.mkdir(parents=True, exist_ok=True, mode=0o700)
        
    async def start(self):
        """Start the Unix socket authentication server"""
        # Remove existing socket if it exists
        if self.socket_path.exists():
            self.socket_path.unlink()
            
        # Create Unix socket server
        self.server = await asyncio.start_unix_server(
            self._handle_auth_request,
            path=str(self.socket_path)
        )
        
        # Set socket permissions (owner only)
        os.chmod(self.socket_path, 0o600)
        
        logger.info(f"Started auth server on Unix socket: {self.socket_path}")
        
    async def stop(self):
        """Stop the authentication server"""
        if self.server:
            self.server.close()
            await self.server.wait_closed()
            
        # Clean up socket file
        if self.socket_path.exists():
            self.socket_path.unlink()
            
        logger.info("Stopped auth server")
        
    async def _handle_auth_request(self, reader: asyncio.StreamReader, 
                                 writer: asyncio.StreamWriter):
        """Handle authentication request from local client"""
        client_addr = writer.get_extra_info('peername')
        logger.debug(f"Auth request from {client_addr}")
        
        try:
            # Read request data
            data = await reader.read(1024)
            if not data:
                await self._send_auth_response(writer, {"error": "No data received"})
                return
                
            try:
                request = json.loads(data.decode('utf-8'))
            except json.JSONDecodeError:
                await self._send_auth_response(writer, {"error": "Invalid JSON"})
                return
                
            # Process authentication request
            if request.get('action') == 'request_token':
                token = self._generate_token(request.get('client_pid'))
                response = {
                    "token": token.token,
                    "expires_at": token.expires_at,
                    "ttl_seconds": self.token_ttl
                }
                await self._send_auth_response(writer, response)
                logger.info(f"Issued token to PID {token.client_pid}")
                
            elif request.get('action') == 'validate_token':
                token_str = request.get('token')
                is_valid = self.validate_token(token_str)
                response = {"valid": is_valid}
                await self._send_auth_response(writer, response)
                
            else:
                await self._send_auth_response(writer, {"error": "Unknown action"})
                
        except Exception as e:
            logger.error(f"Error handling auth request: {e}")
            await self._send_auth_response(writer, {"error": "Internal error"})
        finally:
            writer.close()
            await writer.wait_closed()
            
    async def _send_auth_response(self, writer: asyncio.StreamWriter, response: Dict):
        """Send response to authentication client"""
        try:
            response_data = json.dumps(response).encode('utf-8')
            writer.write(response_data)
            await writer.drain()
        except Exception as e:
            logger.error(f"Error sending auth response: {e}")
            
    def _generate_token(self, client_pid: Optional[int] = None) -> AuthToken:
        """Generate a new authentication token"""
        # Clean up expired tokens first
        self._cleanup_expired_tokens()
        
        # Generate secure random token
        token_str = secrets.token_urlsafe(32)
        
        # Create token object
        current_time = time.time()
        token = AuthToken(
            token=token_str,
            issued_at=current_time,
            expires_at=current_time + self.token_ttl,
            client_pid=client_pid
        )
        
        # Store token
        self.tokens[token_str] = token
        
        return token
        
    def validate_token(self, token_str: str, mark_used: bool = True) -> bool:
        """
        Validate and optionally consume a token
        
        Args:
            token_str: The token string to validate
            mark_used: Whether to mark the token as used (single-use)
        """
        if not token_str or token_str in self.used_tokens:
            return False
            
        token = self.tokens.get(token_str)
        if not token or not token.is_valid():
            return False
            
        if mark_used:
            token.used = True
            self.used_tokens.add(token_str)
            logger.debug(f"Consumed token for PID {token.client_pid}")
            
        return True
        
    def _cleanup_expired_tokens(self):
        """Remove expired tokens from memory"""
        current_time = time.time()
        expired_tokens = [
            token_str for token_str, token in self.tokens.items()
            if current_time >= token.expires_at
        ]
        
        for token_str in expired_tokens:
            del self.tokens[token_str]
            self.used_tokens.discard(token_str)
            
        if expired_tokens:
            logger.debug(f"Cleaned up {len(expired_tokens)} expired tokens")
            
    def get_token_stats(self) -> Dict:
        """Get statistics about current tokens"""
        self._cleanup_expired_tokens()
        
        valid_tokens = sum(1 for token in self.tokens.values() if token.is_valid())
        used_tokens = sum(1 for token in self.tokens.values() if token.used)
        
        return {
            "total_tokens": len(self.tokens),
            "valid_tokens": valid_tokens,
            "used_tokens": used_tokens,
            "expired_tokens": len(self.tokens) - valid_tokens
        }

class AuthClient:
    """Client for requesting authentication tokens"""
    
    def __init__(self, socket_path: Optional[Path] = None):
        if socket_path:
            self.socket_path = Path(socket_path)
        else:
            # Auto-detect socket path
            xdg_runtime = os.getenv('XDG_RUNTIME_DIR')
            if xdg_runtime:
                self.socket_path = Path(xdg_runtime) / 'cardbrick' / 'auth.sock'
            else:
                self.socket_path = Path('/tmp/cardbrick-auth/auth.sock')
                
    async def request_token(self) -> Optional[Dict]:
        """Request an authentication token from the daemon"""
        if not self.socket_path.exists():
            logger.error(f"Auth socket not found: {self.socket_path}")
            return None
            
        try:
            # Connect to Unix socket
            reader, writer = await asyncio.open_unix_connection(str(self.socket_path))
            
            # Send request
            request = {
                "action": "request_token",
                "client_pid": os.getpid()
            }
            
            request_data = json.dumps(request).encode('utf-8')
            writer.write(request_data)
            await writer.drain()
            
            # Read response
            response_data = await reader.read(1024)
            writer.close()
            await writer.wait_closed()
            
            if response_data:
                response = json.loads(response_data.decode('utf-8'))
                return response
            else:
                logger.error("No response from auth server")
                return None
                
        except Exception as e:
            logger.error(f"Error requesting token: {e}")
            return None
            
    async def validate_token(self, token: str) -> bool:
        """Validate a token with the daemon"""
        if not self.socket_path.exists():
            return False
            
        try:
            reader, writer = await asyncio.open_unix_connection(str(self.socket_path))
            
            request = {
                "action": "validate_token",
                "token": token
            }
            
            request_data = json.dumps(request).encode('utf-8')
            writer.write(request_data)
            await writer.drain()
            
            response_data = await reader.read(1024)
            writer.close()
            await writer.wait_closed()
            
            if response_data:
                response = json.loads(response_data.decode('utf-8'))
                return response.get('valid', False)
            else:
                return False
                
        except Exception as e:
            logger.error(f"Error validating token: {e}")
            return False