# Product Overview

faultline is a local-first regression archaeologist for Git repositories.

## Purpose

Given a known-good commit, a known-bad commit, and a predicate (test/build command), faultline narrows the regression window and produces portable artifacts showing where to start investigating.

## Core Promise

1. Walk history safely between good/bad boundaries
2. Run the operator's trusted predicate at candidate revisions
3. Emit JSON + HTML artifacts explaining the narrowest credible regression window

## Key Principles

- **Honest over impressive**: Returns suspect windows when evidence is ambiguous rather than fake precision
- **Predicate-native**: Wraps the predicate operators already trust
- **Local-first**: No external services required
- **Artifact-first**: Always produces portable, inspectable output

## What It Is NOT

- A CI sensor or GitHub bot
- An incident management platform
- An AI root-cause analyzer
- A patch generation system
- A repo topology analyzer
