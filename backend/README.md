# Backend

The backend will serve several responsibilities, including:

1. Serving the frontend static files
2. Allowing manual input from user regarding event type, end of throw, etc. (at least until automation from CV is proven)
3. Running our pipeline from producer to queue to consumers

## Python vs. Rust

Python is great for experimentation, but Rust is a lot better for parallelism, packaging, and performance.

Firstly, Rust has true multi-threaded parallelism within a process, allowing threads to run in parallel on different cores. However, Python has the global interpreter lock (GIL) that only allows one thread at a time to execute Python bytecode. If a thread enters native C code (like for OpenCV), the GIL may be released, but it is still to be determined how much of our code will be in native C.

Secondly, Rust is a compiled-language while Python is an interpreted language. This means that for shipping the final product, Rust would be packaged as a single binary that should run anywhere. On the other hand, Python would be packaged as all our code, a Python runtime, and our dependencies, leading to portability difficulties.

Thirdly, Rust is significantly better than Python for performance. Furthermore, Rust enforces memory and thread safety at compile time.

Due to these differences, our approach will be to experiment with Python to validate our computer vision. However, our final backend will be implemented in Rust.

## Usage

Once the backend is running, you can run the following commands to interact with the backend.

Test server is alive: `curl localhost:5001/api/health`

Get the current throw type: `curl localhost:5001/api/throw-type`

Set the current throw type: `curl -i -X POST localhost:5001/api/throw-type -H "Content-Type: application/json" -d '{"throw_type":"discus"}'`

- Note: This can also be done through the frontend if running the application in integration mode.

## ABOUT PTP

- When recording from one camera, PTP is not used
- When recording with two cameras, PTP sync is established, and we attempt to issue the 'begin capture' commands for both cameras at the same time, but we use a camera-based 30Hz capture instead of using scheduled action commands like the docs suggest.
  - This is because, for some reason, scheduled action command performance has been abysmal and well below the requested framerates, despite validating that the packet construction and scheduling logic is all correct
  - When we send out a 'begin capture' command to both cameras at precisely the same time, and those cameras already have a PTP sync, the cameras use the PTP clock internally to reliably capture at 30Hz. So, the primary source of camera frames being out of sync between left and right cameras is a delay between packets being sent out, and not any clock issues. Using other code tricks, like thread-sync barriers, we can get the 'start capture' packets to be sent out at almost exactly the same time, cutting our camera-frame-sync timing error down to tens of microseconds, which is pretty great all things considered.
  - We have evaluated the performance of no-ptp no-schedule capture, yes-ptp no-schedule capture, and yes-ptp yes-schedule capture, and the best performance by frame rate and frame sync is the yes-ptp no-schedule capture strategy, so that strategy is used when recording or live ingesting from both cameras
