from main import app
from fastapi.routing import APIRoute

print("Registered Routes:")
for route in app.routes:
    if isinstance(route, APIRoute):
        print(f"  {route.methods} {route.path}")
    else:
        # Check for mounted apps
        print(f"  MOUNT {route.path}")
        if hasattr(route, "app"):
            sub_app = route.app
            if hasattr(sub_app, "routes"):
                for sub_route in sub_app.routes:
                    if hasattr(sub_route, "path"):
                         print(f"    -> {sub_route.path}")
