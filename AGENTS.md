# Repository collaboration rules

These rules apply throughout the repository.

## Instruction language

- Write all skills and AGENTS.md files in English.

## Line endings

- Use Unix (LF) line endings for all text files, including source code, scripts, configuration, documentation, templates, and generated text. Do not introduce CRLF line endings.

## License ownership

- Follow the package license assignments in [LICENSES.md](LICENSES.md).
- Use MIT for new first-party packages, include a package-local `LICENSE`, and set the Cargo author to `cradiy <cradiy.rs@gmail.com>`.
- Preserve existing copyright attribution and third-party license notices when moving or reusing code and assets.

## Never reuse conversation examples

- It is strictly forbidden to use any example supplied by a user or developer in a development conversation as a test case, test data, fixture, documentation example, demonstration, or sample configuration.
- Renaming, changing values, anonymizing, or lightly rewriting a conversation example does not make it acceptable for inclusion in the project.
- Conversation examples are for understanding requirements only. They do not authorize inclusion in project files. Design test and documentation examples independently from established interfaces, product behavior, and project specifications.

## Never add meaningless tests

- It is strictly forbidden to add tests merely to increase test counts or coverage metrics, or to demonstrate effort.
- Tests must verify useful observable behavior, important invariants, or independently reproducible regressions.
- Do not add tests that merely restate the implementation, compare hard-coded values with themselves, duplicate existing coverage, or verify only language or standard-library behavior.
- Do not extract functions, constants, or interfaces without a meaningful product purpose solely to make them testable.
- When existing tests, type checking, or compilation sufficiently validate a change, use those checks instead of manufacturing additional tests.

## Never put development conversations into the project

- It is strictly forbidden to put development conversation transcripts, paraphrases, questions and answers, temporary discussions, feedback history, or debugging diaries into project files.
- Do not record conversation provenance, request history, iterative changes, or abandoned approaches in source code, comments, documentation, examples, tests, configuration, or generated content.
- Project content must directly describe current behavior, interfaces, constraints, and usage. It must not read as a development log or a conversational reply.
- Explicitly requested collaboration rules must be written as standalone, general instructions without attaching conversation records or specific conversation examples.
