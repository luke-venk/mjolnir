# Bazel
[Bazel](https://bazel.build/about/intro) is a build tool created by Google that is highly useful for multi-language monorepos like ours. It provides our project the following benefits:
* Standardizes toolchains so all users use the same toolchain instead of their own locally-installed tooling, which varies a lot for compiled languages like Rust
* Incremental builds are used to significantly speed up build time
* Hermetic builds isolates build from host system, ensuring deterministic and reproducible builds across all machines and computer architectures

## Setup
### Mac
See the instructions for installing Bazel on macOS [here](https://bazel.build/install/os-x):
- `brew install bazelisk`  

## Using Bazel
The general format for building a Bazel target (like our executable) is:  
`bazel build //<package>:<target>`  
- `//`: root directory where MODULE.bazel lives
- `<package>`: the directory containing the BUILD.bazel file
- `<target>`: the rule inside the BUILD.bazel file

### Backend
To build our binary, run:  
`bazel build //backend:mjolnir`

To directly run our program, run:  
`bazel run //backend:mjolnir`

To run unit tests, run:  
`bazel test //backend:tests`  

### Frontend
#### Run Dev Server
To run the dev server, run:
`bazel run //frontend:dev`  
