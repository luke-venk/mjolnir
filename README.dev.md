# Developer README

## Backend Overview
The backend will serve several responsibilities, including:
1. Serving the frontend static files
2. Allowing manual input from user regarding event type, end of throw, etc. (at least until automation from CV is proven)
3. Running our pipeline from producer to queue to consumers

### Python vs. Rust
Python is great for experimentation, but Rust is a lot better for parallelism, packaging, and performance.

Firstly, Rust has true multi-threaded parallelism within a process, allowing threads to run in parallel on different cores. However, Python has the global interpreter lock (GIL) that only allows one thread at a time to execute Python bytecode. If a thread enters native C code (like for OpenCV), the GIL may be released, but it is still to be determined how much of our code will be in native C.

Secondly, Rust is a compiled-language while Python is an interpreted language. This means that for shipping the final product, Rust would be packaged as a single binary that should run anywhere. On the other hand, Python would be packaged as all our code, a Python runtime, and our dependencies, leading to portability difficulties.

Thirdly, Rust is significantly better than Python for performance. Furthermore, Rust enforces memory and thread safety at compile time.

Due to these differences, the current plan is to experiment with Python and validate our computer vision. However, our actual pipeline will be implemented in Rust.


## Rust Backend
### Usage
Run unit tests:  `cargo test`  

Run backend:  `cargo run`

### Routes
Test server is alive:  `curl localhost:5001/health`

Get the current event:  `curl localhost:5001/throw-type`

Set the current event:  `curl -i -X POST localhost:5001/throw-type -H "Content-Type: application/json" -d '{"throw_type":"discus"}'`
- Note: This can also be done through the frontend if running the application in integration mode.
