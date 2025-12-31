# Ghostty VT WASM

This project uses Ghostty's `lib-vt` Zig module to produce a custom
`ghostty-vt.wasm` for the renderer. The build is pinned to a specific Ghostty
commit and applies a small API patch so the wasm exports include the terminal
render-state functions expected by `ghostty-web`.

## Build

From the repo root:

```bash
./tools/ghostty-vt-wasm/build-wasm.sh
```

That script will:
- Clone Ghostty at the pinned commit into `tools/ghostty-vt-wasm/.ghostty`
- Apply `tools/ghostty-vt-wasm/patches/ghostty-wasm-api.patch`
- Build `lib-vt` for `wasm32-freestanding`
- Copy the output to `src/assets/wasm/ghostty-vt.wasm`

## Updating Ghostty

1. Update `GHOSTTY_COMMIT` in `tools/ghostty-vt-wasm/build-wasm.sh`.
2. Re-run the build script.
3. If the patch no longer applies cleanly, update
   `tools/ghostty-vt-wasm/patches/ghostty-wasm-api.patch` accordingly.

## Output

- `src/assets/wasm/ghostty-vt.wasm` is the renderer's pinned wasm module.
