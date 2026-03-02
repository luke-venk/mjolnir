# The producer will handle ingesting the frames from the cameras and enqueue
# into the queue. The producer has the following responsbilities:
# (1) Pull frames from each camera

from pipeline.queue import MyQueue as Queue

class Producer:
    def __init__(self, q: Queue):
        self.q = q
