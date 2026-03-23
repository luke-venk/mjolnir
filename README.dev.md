# Developer README
## Software Stack
We are using a monorepo, which in our case is nice since our project features a variety of languages. See the attached READMEs for more information on how to use each component:
- [Backend](/backend/README.md): Rust
- [Frontend](/frontend/README.md): TypeScript (Next.js)
- [Build](/README.bazel.md): Bazel
- Experimentation: Python and MATLAB (just for validation, see [here](/backend/README.md#python-vs-rust))

## Setup
### Bazel (IMPORTANT)
See [README.bazel.md](/README.bazel.md) for more information on why we are using the Bazel build system for this project.  

Refer to the [Usage section](/README.bazel.md#usage) for the commands to build and run the project.

Before committing any changes, ensure that the entire project builds: `bazel build //...`  

### VS Code Extensions
Please use VS Code for this project. Otherwise, you are a freak. Also, install the following extensions for a smooth experience:  
- [Bazel](vscode:extension/BazelBuild.vscode-bazel)
- [Rust Analyzer](vscode:extension/rust-lang.rust-analyzer)

### Scripts
In the repository roots, there are scripts to install the dependencies needed to develop software for this project.  

If you are using Mac, run:  `./scripts/setup_mac.sh`

If you are using Linux, run:  `./scripts/setup_linux.sh`

We aren't supporting development for Windows. If you are using Windows, please use Windows Subsystem for Linux (WSL) and follow the instructions provided for Linux.
