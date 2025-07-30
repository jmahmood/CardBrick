#!/usr/bin/env python3
"""
Generate OpenAPI client for CardBrick Sync Daemon

This script extracts the OpenAPI specification from the running daemon
and generates Python client code.
"""

import json
import subprocess
import sys
from pathlib import Path
from typing import Optional

import requests

def get_openapi_spec(daemon_host: str = "localhost", daemon_port: int = 6429) -> dict:
    """Fetch OpenAPI specification from running daemon"""
    url = f"http://{daemon_host}:{daemon_port}/openapi.json"
    
    try:
        response = requests.get(url, timeout=10)
        response.raise_for_status()
        return response.json()
    except requests.RequestException as e:
        print(f"Error fetching OpenAPI spec: {e}")
        print("Make sure the CardBrick sync daemon is running")
        sys.exit(1)

def save_openapi_spec(spec: dict, output_path: Path) -> None:
    """Save OpenAPI specification to file"""
    with open(output_path, 'w', encoding='utf-8') as f:
        json.dump(spec, f, indent=2, ensure_ascii=False)
    print(f"Saved OpenAPI spec to: {output_path}")

def generate_python_client(spec_path: Path, output_dir: Path) -> None:
    """Generate Python client using openapi-generator"""
    # Check if openapi-generator is available
    try:
        subprocess.run(['openapi-generator-cli', 'version'], 
                      capture_output=True, check=True)
        generator_cmd = 'openapi-generator-cli'
    except (subprocess.CalledProcessError, FileNotFoundError):
        try:
            subprocess.run(['openapi-generator', 'version'], 
                          capture_output=True, check=True)
            generator_cmd = 'openapi-generator'
        except (subprocess.CalledProcessError, FileNotFoundError):
            print("Error: openapi-generator not found")
            print("Install with: npm install -g @openapitools/openapi-generator-cli")
            print("Or download from: https://openapi-generator.tech/docs/installation")
            return

    # Generate Python client
    cmd = [
        generator_cmd, 'generate',
        '-i', str(spec_path),
        '-g', 'python',
        '-o', str(output_dir),
        '--package-name', 'cardbrick_sync_client',
        '--project-name', 'cardbrick-sync-client',
        '--additional-properties', 'packageVersion=0.1.0'
    ]
    
    try:
        subprocess.run(cmd, check=True)
        print(f"Generated Python client in: {output_dir}")
    except subprocess.CalledProcessError as e:
        print(f"Error generating client: {e}")

def main():
    """Main entry point"""
    # Set up paths
    script_dir = Path(__file__).parent
    output_dir = script_dir / "generated_client"
    spec_path = script_dir / "openapi.json"
    
    # Parse command line arguments
    daemon_host = sys.argv[1] if len(sys.argv) > 1 else "localhost"
    daemon_port = int(sys.argv[2]) if len(sys.argv) > 2 else 6429
    
    print(f"Fetching OpenAPI spec from {daemon_host}:{daemon_port}")
    
    # Fetch and save OpenAPI specification
    spec = get_openapi_spec(daemon_host, daemon_port)
    save_openapi_spec(spec, spec_path)
    
    # Generate Python client
    print("Generating Python client...")
    generate_python_client(spec_path, output_dir)
    
    # Create simple usage example
    example_path = output_dir / "example_usage.py"
    example_code = '''#!/usr/bin/env python3
"""
Example usage of the generated CardBrick sync client
"""

from cardbrick_sync_client import ApiClient, Configuration
from cardbrick_sync_client.api.default_api import DefaultApi

# Configure client
configuration = Configuration(host="http://localhost:6429")
client = ApiClient(configuration)
api = DefaultApi(client)

# Example: Get daemon status
try:
    status = api.status()
    print("Daemon Status:")
    print(f"  Version: {status.daemon['version']}")
    print(f"  Uptime: {status.daemon['uptime_seconds']} seconds")
    print(f"  Active jobs: {status.jobs['active']}")
except Exception as e:
    print(f"Error: {e}")

# Example: Get health check
try:
    health = api.health()
    print(f"Health: {health.status}")
except Exception as e:
    print(f"Error: {e}")
'''
    
    with open(example_path, 'w') as f:
        f.write(example_code)
    
    print(f"Created example usage: {example_path}")
    print("\nNext steps:")
    print("1. Install the generated client: pip install -e generated_client/")
    print("2. Run the example: python generated_client/example_usage.py")

if __name__ == "__main__":
    main()