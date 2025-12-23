import sys
import json
import argparse
from lib.api_auth import get_test_client

def main():
    parser = argparse.ArgumentParser(description='Query authenticated API endpoints')
    parser.add_argument('method', choices=['GET', 'POST', 'DELETE'], help='HTTP method')
    parser.add_argument('path', help='API path (e.g., /api/v1/private/orders)')
    parser.add_argument('--user', type=int, default=1001, help='User ID')
    parser.add_argument('--data', help='JSON data for POST')
    parser.add_argument('--params', help='JSON query parameters')
    
    args = parser.parse_args()
    
    client = get_test_client(user_id=args.user)
    
    params = json.loads(args.params) if args.params else None
    data = json.loads(args.data) if args.data else None
    
    if args.method == 'GET':
        resp = client.get(args.path, params=params)
    elif args.method == 'POST':
        resp = client.post(args.path, json_body=data)
    elif args.method == 'DELETE':
        resp = client.delete(args.path, json_body=data)
    else:
        print(f"Unsupported method: {args.method}")
        sys.exit(1)

    try:
        # Return JSON if possible
        result = resp.json()
    except:
        # Fallback to status code and text
        result = {
            "status_code": resp.status_code,
            "error": "Non-JSON response",
            "text": resp.text[:200]
        }
        
    print(json.dumps(result))

if __name__ == "__main__":
    main()
