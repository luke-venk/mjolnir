# Bazel
[Bazel](https://bazel.build/about/intro) is a build tool created by Google that is highly useful for multi-language monorepos like ours. It provides our project the following benefits:
* Standardizes toolchains so all users use the same toolchain instead of their own locally-installed tooling, which varies a lot for compiled languages like Rust
* Incremental builds are used to significantly speed up build time
* Hermetic builds isolates build from host system, ensuring deterministic and reproducible builds across all machines and computer architectures

## External Depedencies
This Rust project doesn't use Cargo, instead specifying dependencies through Bazel. To add a dependency, similar to how you would normally add a dependency in Cargo.toml, specify dependencies in [MODULE.bazel](MODULE.bazel) using `crate.spec(package = "my_package", version = "1.2.3")`. Then, include them in the `deps` argument of your Rust target like `"@crates//:package_name"`.

## Backend Configurations
There will be 4 possible targets for the backend binary found in [backend/BUILD.bazel](backend/BUILD.bazel):
1. Dev Fake
2. Dev Real
3. Prod Fake
4. Prod Real

### Dev vs. Prod
The difference between development (dev) and production (prod) mode is that _dev_ mode relies on Next.js on port 3000 to run the frontend, while _prod_ mode statically exports the frontend and embeds it directly into the binary. These significantly different implementations require different entry points.

### Fake vs. Real
The difference between fake and real is that _fake_ mode uses simulated throw data, while _real_ mode actually starts up the per-camera computer vision pipeline and processes real frames to determine where the throw landed. We just use conditional compilation via `#[cfg(feature)]` flags to separate the functionality.

## Usage
There are 4 ways we would want Bazel to build/run our project:
1. Frontend-only dev
2. Backend-only dev
3. Integration dev
4. Production mode

The general format for building a Bazel target (like our executable) is:  
`bazel build //<package>:<target>`  
- `//`: root directory where MODULE.bazel lives
- `<package>`: the directory containing the BUILD.bazel file
- `<target>`: the rule inside the BUILD.bazel file

### (1) Frontend-only dev
In this scenario, we would have Next.js serve both the frontend and backend, so Rust would not be involved at all. This provides hot-module reload and quick testing for our frontend. You can interact with the frontend in your browser at `localhost:3000`.  

To run the frontend alone with simulated data, run:  
`bazel run //frontend:dev-fake`  

### (2) Backend-only dev
In this scenario, we would only have the Axum server and Rust running the backend. No frontend would be used for this. You can interact with the backend through the command line using curl, instructions for which are found in the [backend README](/backend/README.md#usage).  

To run the backend using purely simulated throw and circle infraction data, run:  
`bazel run //backend:dev_all_fake`    

To run the backend using real throw data but simulated circle infraction data, ensure cameras are connected and run:  
`bazel run //backend:dev_real_cameras`  

To run the backend using real circle infraction data but simulated throw data, ensure the Arduino is connected and run:  
`bazel run //backend:dev_real_circle_sensors`  

To run the backend using purely real data for both throws and circle infractions, ensure all hardware is connected and run:  
`bazel run //backend:dev_all_real`  

To run unit tests, run:  
`bazel test //backend:tests`  

### (3) Integration dev
In this scenario, we would run both Next.js for frontend and Axum for backend. We would run Next.js on port 3000 and Axum on port 5001. You can interact with the frontend in your browser at `localhost:3000` and confirm the throw events are updated in the backend through the command line.  

To run in integration mode, start whichever backend mode you want using one of the 4 previous run commands (in section 2), and run the following command:  
`bazel run //frontend:integration`  

### (4) Production Mode
The final production build uses the prod target with the release Bazel config (found in .bazelrc) to optimize the backend build and serve the embedded frontend assets. When running this, you can open your browser to `localhost:5001` and interact with the application.  

To run the backend using purely simulated throw and circle infraction data, run:  
`bazel run //backend:prod_all_fake`    

To run the backend using real throw data but simulated circle infraction data, ensure cameras are connected and run:  
`bazel run //backend:prod_real_cameras`  

To run the backend using real circle infraction data but simulated throw data, ensure the Arduino is connected and run:  
`bazel run //backend:prod_real_circle_sensors`  

To run the backend using purely real data for both throws and circle infractions, ensure all hardware is connected and run:  
`bazel run //backend:prod_all_real`  

To ensure that release optimizations are applied to the binary, build any of the previous 4 targets with the `--config=release` flag.  
e.g,  `bazel build --config=release //backend:prod_all_real`

The final product would be the binary found in `bazel-bin/backend/<the target you built>`.
