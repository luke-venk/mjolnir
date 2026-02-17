"""
Definitions of the HTTP endpoints for the API to use.
"""
from fastapi import APIRouter
from api.schema import BackendToFrontend
from api.dummy import get_dummy_data, save_result, create_frames  # TODO: remove

router = APIRouter()

@router.get("/hello_world")
def hello_world() -> dict:
    return {"ok": True, "message": "Hello World!"}

@router.get("/dummy", response_model=BackendToFrontend, response_model_by_alias=True)
def dummy():
    """
    Return dummy data for testing frontend integration. Also saves results
    to volume mount so frontend can pull from it.
    """
    # TODO: remove once dummy not neeeded
    dummy_data = get_dummy_data()
    save_result(dummy_data)
    create_frames(dummy_data)
    return dummy_data
    

@router.get("/throws/latest", response_model=BackendToFrontend)
def latest():
    """
    Returns the results of the latest throw published by the backend.
    """
    # TODO: update this from dummy
    return dummy()
