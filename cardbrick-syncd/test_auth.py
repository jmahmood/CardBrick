#!/usr/bin/env python3
"""
Test script for local authentication system
"""

import asyncio
import sys

from auth import AuthClient

async def test_auth():
    """Test the authentication system"""
    print("Testing CardBrick local authentication...")
    
    # Create auth client
    client = AuthClient()
    
    print(f"Using socket path: {client.socket_path}")
    
    # Request a token
    print("Requesting authentication token...")
    token_response = await client.request_token()
    
    if not token_response:
        print("❌ Failed to get token")
        return False
        
    if 'error' in token_response:
        print(f"❌ Error: {token_response['error']}")
        return False
        
    print(f"✓ Received token: {token_response['token'][:16]}...")
    print(f"  TTL: {token_response['ttl_seconds']} seconds")
    
    # Validate the token
    print("Validating token...")
    is_valid = await client.validate_token(token_response['token'])
    
    if is_valid:
        print("✓ Token is valid")
    else:
        print("❌ Token validation failed")
        return False
        
    # Try to validate the same token again (should fail - single use)
    print("Testing single-use behavior...")
    is_valid_again = await client.validate_token(token_response['token'])
    
    if not is_valid_again:
        print("✓ Token correctly marked as used (single-use)")
    else:
        print("❌ Token should be single-use only")
        return False
        
    print("✓ All authentication tests passed!")
    return True

def main():
    """Main entry point"""
    try:
        result = asyncio.run(test_auth())
        sys.exit(0 if result else 1)
    except KeyboardInterrupt:
        print("\nTest interrupted")
        sys.exit(1)
    except Exception as e:
        print(f"Test failed with error: {e}")
        sys.exit(1)

if __name__ == "__main__":
    main()