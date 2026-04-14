# Cloud Agent Development

When local Rust tooling is unavailable in a cloud-agent environment, GitHub Actions is the verification oracle for this repository.

## Required workflow

1. Branch from the current `main` branch.
2. Make surgical changes only.
3. Push the branch to GitHub.
4. Wait for the `Rust CI` workflow to run.
5. Fix failures until the workflow is green.
6. Only then open or update a pull request.

## Required verification gate

Do not claim the repository is ready unless all of the following GitHub Actions jobs are green:

- `fmt`
- `clippy`
- `check`
- `test`

## Working rules

- Always work from the current repository state.
- Do not reconstruct large files from partial snippets or memory.
- Use GitHub Actions results as the source of truth when local `cargo` or `rustc` is unavailable.
