"""
Used for testing integration with frontend. Just returns the backend to
frontend JSON response with dummy data.
"""
from api.schema import LandingPoint, Image, Infraction, BackendToFrontend
from uuid import uuid4
from datetime import datetime, timezone
from pathlib import Path  # TODO: remove
import shutil  # TODO: remove

def get_dummy_data():
    throw_id = uuid4()
    
    return BackendToFrontend(
        throw_id=throw_id,
        timestamp=datetime.now(tz=timezone.utc),
        distance=69.18,
        images=[
            # Image from far left camera.
            Image(
                url=f"/media/{throw_id}/image1.jpg",
                landing_point=LandingPoint(x=100, y=200)
            ),
            # Image from far right camera.
            Image(
                url=f"/media/{throw_id}/image2.jpg",
                landing_point=LandingPoint(x=300, y=200)
            ),
            # Image from near right camera.
            Image(
                url=f"/media/{throw_id}/image3.jpg",
                landing_point=LandingPoint(x=100, y=500)
            )
        ],
        infractions=[
            Infraction(
                type="sector_foul",
                confidence=0.67
            )
        ]
    )
    
def save_result(result: BackendToFrontend):
    """
    Save the result to data/ so frontend can pull results and media from it.
    """
    # TODO: move this since saving would happen somewhere else for our real pipeline
    throw_dir = Path("/data") / str(result.throw_id)
    throw_dir.mkdir(parents=True, exist_ok=True)
    with open(throw_dir / "result.json", "w") as f:
        f.write(result.model_dump_json(by_alias=True, indent=2))
        
def create_frames(result: BackendToFrontend):
    """
    Actually save dummy frame images in the volume mount so frontend can
    call it. save_result just linked the URL in result.json.
    """
    throw_dir = Path("/data") / str(result.throw_id)
    throw_dir.mkdir(parents=True, exist_ok=True)
    
    # Path of dummy image inside container.
    src_img = Path("/app/api/dummy.jpeg")
    
    # Save the 3 frames.
    shutil.copy(src_img, throw_dir / "image1.jpg")
    shutil.copy(src_img, throw_dir / "image2.jpg")
    shutil.copy(src_img, throw_dir / "image3.jpg")
