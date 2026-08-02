# strata-render

Renderer-neutral scene primitives and source-picking contracts. Views publish
tiles, points, lines, rectangles, meshes, labels, and volume slices with exact,
sampled, aggregate, or approximate coverage. The live `egui` canvas currently
implements its own presentation path while the production renderer is promoted.
