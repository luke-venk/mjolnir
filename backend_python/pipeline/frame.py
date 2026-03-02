from datetime import datetime

class Frame:
    def __init__(self, camera_id: int, frame_id: int, timestamp: datetime):
        self.camera_id = camera_id
        self.frame_id = frame_id
        self.timestamp = timestamp
