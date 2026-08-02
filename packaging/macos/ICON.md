# Strata icon

`Strata.icon-master.png` is the 1024 × 1024 source for the checked-in
`Strata.icns`. It was generated with OpenAI's built-in image-generation tool
on 2026-08-02, selected by the maintainer workflow, resized, given a transparent
rounded-square matte, and stripped of ancillary metadata locally.

The production prompt was:

> Create a premium macOS application icon for Strata, a professional binary
> analysis workbench. A single centered dark graphite rounded-square tile
> contains a compact abstract 3D monolith made of crisp stacked pixel and voxel
> strata. Restrained cyan and teal data layers cross the charcoal body, with
> one tiny amber evidence accent and neutral pale highlights. Use a strong,
> simple silhouette readable at 16 pixels, controlled studio relief, precise
> geometric rendering, and a mature developer-tool aesthetic. Do not include
> text, letters, numbers, watermarks, an outer scene, lens flare, bloom, neon
> haze, noisy particles, a magnifying glass, shield, database cylinder, cartoon
> styling, glossy plastic, or visual clutter.

Regenerate the multi-resolution container without changing the master:

```bash
bash scripts/generate-macos-icon.sh
```

Any future replacement should preserve transparent corners, remain readable at
16 pixels, and update both the master and `.icns` in the same reviewed change.
