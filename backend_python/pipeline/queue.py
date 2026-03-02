# Implementation of the producer-consumer queue used by our pipeline.
# There will be one queue for each pipeline (i.e. one queue for each
# of the 2 cameras).
#
# While Python doesn't really enable multi-threaded parallelism due to
# its GIL, to use thread safe queues, we will utilize Queue module
# to implement thread-safe producer-consumer queues. The documentation
# for Queue can be found here:
# https://docs.python.org/3/library/queue.html
#
# We will use a bounded channel to maintain a rolling buffer storing up
# to 10 frames. We will also use a drop-oldest policy so that, if the queue
# fills up while analyzing the frames, we will drop the oldest frame, as
# to avoid working on analyzing stale data.
from queue import Queue, Full, Empty

from pipeline.frame import Frame

class MyQueue:
    def __init__(self, capacity: int):
        self.q = Queue(maxsize=capacity)
        
    def enqueue(self, frame: Frame) -> None:
        try:
            # Send the frame.
            self.q.put_nowait(frame)
        except Full:
            # If the queue is full, drop the oldest frame in favor of the 
            # new one.
            try:
                self.q.get_nowait()
            except Empty:
                # In this case, if between enqueueing and dequeuing, the queue
                # is empty, a producer probably just dequeued the last frame.
                # This is a rare race condition.
                pass
            # Finally, try to resend the frame.
            self.q.put_nowait(frame)

    def dequeue(self) -> Frame:
        # Block until a frame is available, then pop and return it.
        return self.q.get()
