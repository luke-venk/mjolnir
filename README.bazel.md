# Bazel
[Bazel](https://bazel.build/about/intro) is a build tool created by Google that is highly useful for multi-language monorepos like ours. It provides our project the following benefits:
* Standardizes toolchains so all users use the same toolchain instead of their own locally-installed tooling, which varies a lot for compiled languages like Rust
* Incremental builds are used to significantly speed up build time
* Hermetic builds isolates build from host system, ensuring deterministic and reproducible builds across all machines and computer architectures

## External Depedencies
This Rust project doesn't use Cargo, instead specifying dependencies through Bazel. To add a dependency, similar to how you would normally add a dependency in Cargo.toml, specify dependencies in [MODULE.bazel](MODULE.bazel) using `crate.spec(package = "my_package", version = "1.2.3")`. Then, include them in the `deps` argument of your Rust target like `"@crates//:package_name"`.

## Usage - Main Server
There are 4 ways we would want Bazel to build/run our project:
1. Next.js-only dev
3. Rust-only dev
2. Integration dev
4. Production

The general format for building a Bazel target (like our executable) is:  
`bazel build //<package>:<target>`  
- `//`: root directory where MODULE.bazel lives
- `<package>`: the directory containing the BUILD.bazel file
- `<target>`: the rule inside the BUILD.bazel file

### (1) Next.js-only dev
In this scenario, we would have Next.js serve both the frontend and backend, so Rust would not be involved at all. This provides hot-module reload and quick testing for our frontend. You can interact with the frontend in your browser at `localhost:3000`.  

To run the frontend, run:  
`bazel run //frontend:dev`  

## (2) Rust-only dev
In this scenario, we would only have the Axum server and Rust running the backend. No frontend would be used for this. You can interact with the backend through the command line using curl, instructions for which are found in the [backend README](/backend/README.md#usage).  

To build or run the backend, run:  
`bazel build //backend:dev`
`bazel run //backend:dev`

To run unit tests, run:  
`bazel test //backend:tests`  

### (3) Integration dev
In this scenario, we would run both Next.js for frontend and Axum for backend. We would run Next.js on port 3000 and Axum on port 5001. You can interact with the frontend in your browser at `localhost:3000` and confirm the throw events are updated in the backend through the command line.  

To run the integration dev servers, run both commands in separate terminals:  
`bazel run //backend:dev`  
`bazel run //frontend:integration`  

### (4) Production
The final production build uses the prod target with the release Bazel config (found in .bazelrc) to optimize the backend build and serve the embedded frontend assets. When running this, you can open your browser to `localhost:5001` and interact with the application.  

To build or run the final product, run:  
`bazel build --config=release //backend:prod`  
`bazel run --config=release //backend:prod`  

The final product would be the binary found in `bazel-bin/backend/prod`.

## Usage - Discover Cameras
To build the auxiliary binary for discovering cameras on the LAN, run the following command:  
`bazel build //backend:discover_cameras`  

## Usage - Record from Cameras
To run the H.265 camera recorder and optionally recover PNGs in the same Bazel run, use:
```
bazel run //backend:record_from_cameras -- --camera "Lucid Vision Labs-ATP124S-M-224300917" --resolution 4k --exposure-us 25.4 --frame-rate-hz 30 --save-recordings-dir ~/Downloads/camera_out --recover-to-png-dir ~/Downloads/camera_png --max-frames 100
```
This writes one lossless `.h265` stream per camera into `--save-recordings-dir`, and if `--recover-to-png-dir` is provided it also decodes the compressed stream back into `frame_000000.png`, `frame_000001.png`, and so on in the same run.

If you still need the original raw `.raw` + `.json` behavior, it remains available as:
```
bazel run //backend:record_from_cameras_raw -- --camera "Lucid Vision Labs-ATP124S-M-224300917" --resolution 4k --exposure-us 25.4 --frame-rate-hz 30 --save-recordings-dir ~/Downloads/camera_out --max-frames 100
```
The legacy H.265 alias targets below point at the same recorder:
```
bazel run //backend:record_from_cameras_h265 -- --camera "Lucid Vision Labs-ATP124S-M-224300917" --resolution 4k --exposure-us 25.4 --frame-rate-hz 30 --save-recordings-dir ~/Downloads/camera_out --recover-to-png-dir ~/Downloads/camera_png --max-frames 100

bazel run //backend:record_cameras -- --camera "Lucid Vision Labs-ATP124S-M-224300917" --resolution 4k --exposure-us 25.4 --frame-rate-hz 30 --save-recordings-dir ~/Downloads/camera_out --recover-to-png-dir ~/Downloads/camera_png --max-frames 100
```

System requirement: `ffmpeg` must be installed on the host and built with `libx265` support. If it is not on `PATH`, set `MJOLNIR_FFMPEG_BIN` to the full executable path before running the binary.

## Usage - Recover Lossless H.265 to PNG
To decode a previously recorded `.h265` stream back into PNG frames, run:
```
bazel run //backend:recover_h265_to_png -- --h265-path ~/Downloads/camera_out/Lucid_Vision_Labs-ATP124S-M-224300917/Lucid_Vision_Labs-ATP124S-M-224300917.h265 --output-dir ~/Downloads/camera_out/recovered
```

Recovery inspects the H.265 SPS with `scuffle-h265` before decoding and then uses `ffmpeg` to write `frame_000000.png`, `frame_000001.png`, and so on. Use a new or empty output directory for recovery so the generated PNG count is unambiguous.
