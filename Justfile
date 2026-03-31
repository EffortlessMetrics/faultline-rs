default:
    @just --list

ci: ci-fast

ci-fast:
    cargo xtask ci-fast

ci-full:
    cargo xtask ci-full

smoke:
    cargo xtask smoke

golden:
    cargo xtask golden

mutants:
    cargo xtask mutants

fuzz duration="60":
    cargo xtask fuzz --duration {{duration}}

docs:
    cargo xtask docs-check

release-check:
    cargo xtask release-check

scaffold *args:
    cargo xtask scaffold {{args}}
