# CardBrick Documentation

This directory provides additional details beyond the README design spec.

## Planned Phases

1. **Environment Setup** – configure the cross‑compile toolchain and basic build system.
2. **Core Deck Logic** – implement flashcard structures and persistence.
3. **UI Integration** – display cards on Trim UI Brick and handle input.
4. **Polish and Deployment** – refine interactions and package the app.

## Building the Project

CardBrick uses a standard CMake workflow. To build on a development machine:

```bash
cmake -S . -B build
cmake --build build
```

See the [design spec](../README.md#design-spec) in the README for architecture details and future plans.
