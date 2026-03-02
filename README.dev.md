# Developer README

## Backend
The backend will serve several responsibilities, including:
1. Serving the frontend static files
2. Allowing manual input from user regarding event type, end of throw, etc. (at least until automation from CV is proven)
3. Running our pipeline from producer to queue to consumers

### Python vs. Rust
Python is great for experimentation, but Rust is a lot better for parallelism, packaging, and performance.

Firstly, Rust has true multi-threaded parallelism within a process, allowing threads to run in parallel on different cores. However, Python has the global interpreter lock (GIL) that only allows one thread at a time to execute Python bytecode. If a thread enters native C code (like for OpenCV), the GIL may be released, but it is still to be determined how much of our code will be in native C.

Secondly, Rust is a compiled-language while Python is an interpreted language. This means that for shipping the final product, Rust would be packaged as a single binary that should run anywhere. On the other hand, Python would be packaged as all our code, a Python runtime, and our dependencies, leading to portability difficulties.

Thirdly, Rust is significantly better than Python for performance. Furthermore, Rust enforces memory and thread safety at compile time.

Due to these differences, the current plan is to experiment with Python and validate our pipeline. In the end, if time permits, we will rewrite the backend in Rust.

## Python Backend
### Usage
Run everything with:  `make`
Bring everything down with:  `make down`

Check Makefile to see what these do, but basically just using Docker Compose.

### Getting Dummy Data from Backend
Test server is alive:  `curl localhost:8000/api/hello_world`

Get dummy data:  `curl localhost:8000/api/dummy`
* Saves data to data/
* Both results.json (which provides the URLs to the images), as well as the images

Get images from a given throw:  `curl localhost:8000/media/<UUID>/image<1|2|3>.jpg -o <output>.jpg`

### Testing
To run the unit tests using `pytest`, run the following command from `mjolnir/backend_python`:
```bash
PYTHONPATH=. pytest -q
```

## Rust Backend
### Usage
