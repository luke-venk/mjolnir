"""Pydantic models for FastAPI to use."""
from pydantic import BaseModel, Field, ConfigDict
from typing import Literal
from uuid import UUID
from datetime import datetime

# The landing point of the object in pixels for each image.
class LandingPoint(BaseModel):
    x: float
    y: float
    
# As a given throw will have 3 images (one from each camera), each one
# should have a URL associated with it so the frontend can fetch the image,
# as well as a landing point (in px).
class Image(BaseModel):
    model_config = ConfigDict(populate_by_name=True, serialize_by_alias=True)
    
    url: str
    landing_point: LandingPoint = Field(alias="landingPoint")
    
# An infraction can either be a `foot_fault` or a `sector_foul`. A fault
# will also have some confidence associated with it (TODO: update confidence
# to be optional if circle infraction system does NOT use ML).
class Infraction(BaseModel):
    type: Literal["foot_fault", "sector_foul"]
    confidence: float

# The JSON message format the frontend will poll from the backend.
class BackendToFrontend(BaseModel):
    model_config = ConfigDict(populate_by_name=True, serialize_by_alias=True)
    
    throw_id: UUID = Field(alias="throwId")
    timestamp: datetime
    distance: float
    images: list[Image]
    infractions: list[Infraction]
