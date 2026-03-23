# Bazel
[Bazel](https://bazel.build/about/intro) is a build tool created by Google that is highly useful for multi-language monorepos like ours. It provides our project the following benefits:
* Standardizes toolchains so all users use the same toolchain instead of their own locally-installed tooling, which varies a lot for compiled languages like Rust
* Incremental builds are used to significantly speed up build time
* Hermetic builds isolates build from host system, ensuring deterministic and reproducible builds across all machines and computer architectures

## External Depedencies
This Rust project doesn't use Cargo, instead specifying dependencies through Bazel. To add a dependency, similar to how you would normally add a dependency in Cargo.toml, specify dependencies in [MODULE.bazel](MODULE.bazel) using `crate.spec(package = "my_package", version = "1.2.3")`. Then, include them in the `deps` argument of your Rust target like `"@crates//:package_name"`.

## Usage
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
In this scenario, we would have Next.js serve both the frontend and backend, so Rust would not be involved at all. This provides hot-module reload and quick testing for our frontend.

To run the frontend, run:  
`bazel run //frontend:dev`  

## (2) Rust-only dev
In this scenario, we would only have the Axum server and Rust running the backend. No frontend would be used for this.  

To build the backend, run:  
`bazel build //backend:mjolnir`

To directly run the backend, run:  
`bazel run //backend:mjolnir`

To run unit tests, run:  
`bazel test //backend:tests`  

### (3) Integration dev
In this scenario, we would run both Next.js for frontend and Axum for backend. We would run Next.js on port 3000 and Axum on port 5001.  

To run the integration dev servers, run both commands in separate terminals:  
`bazel run //backend:mjolnir`  
`bazel run //frontend:integration`  

### (4) Production
The final production build will be a single build target that sets optimization flags on the Rust build and grabs the static export from frontend as assets for the Rust binary to serve.  

#### Run Dev Server
To build the final product, run:  
`bazel build TODO`  

To build the frontend's static exports, run the following command. Note that this is automatically done when the final product is built:  
`bazel build //frontend:static_exports`  
