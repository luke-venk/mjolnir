"""
Entry point for uvicorn server.
"""
from fastapi import FastAPI, staticfiles
from pathlib import Path
from api.routes import router as api_router

app = FastAPI(title="Mjolnir")

# Use router for JSON API calls.
app.include_router(api_router, prefix="/api")

# Serve media with StaticFiles mounted at /media.
# The filesystem path will be /data/throws inside the container.
app.mount("/media", staticfiles.StaticFiles(directory="/data"), name="media")

# Create local data directory.
Path("/data").mkdir(parents=True, exist_ok=True)
