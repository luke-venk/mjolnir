from datetime import datetime

from pipeline.queue import MyQueue as Queue
from pipeline.frame import Frame

def test_enqueue_then_dequeue_one():
    q = Queue(capacity=3)
    q.enqueue(Frame(camera_id=1, frame_id=1, timestamp=datetime.now()))
    frame = q.dequeue()
    
    assert frame.camera_id == 1
    assert frame.frame_id == 1

def test_drop_oldest_when_full():
    q = Queue(capacity=3)
    q.enqueue(Frame(camera_id=2, frame_id=1, timestamp=datetime.now()))
    q.enqueue(Frame(camera_id=2, frame_id=2, timestamp=datetime.now()))
    q.enqueue(Frame(camera_id=2, frame_id=3, timestamp=datetime.now()))
    q.enqueue(Frame(camera_id=2, frame_id=4, timestamp=datetime.now()))
    
    frame1 = q.dequeue()
    frame2 = q.dequeue()
    frame3 = q.dequeue()
    
    assert frame1.frame_id == 2
    assert frame2.frame_id == 3
    assert frame3.frame_id == 4
