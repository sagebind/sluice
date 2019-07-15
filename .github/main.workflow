workflow "build" {
  on = "push"
  resolves = ["test-nightly"]
}

action "test-nightly" {
  uses = "docker://rustlang/rust:nightly"
  args = "cargo test --features nightly"
}
