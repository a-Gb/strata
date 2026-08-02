# Example external WASM analyzer

This directory is a design shell for a Tier-1 component plugin. A future example should:

1. implement `wit/strata-plugin.wit`;
2. request only `source.metadata`, bounded `source.ranges`, and `findings.emit`;
3. scan an approved selection for a simple deterministic byte pattern;
4. emit exact range findings;
5. checkpoint no sensitive state;
6. include a signed manifest and quota declaration;
7. include host tests for denial, timeout, oversized output, and stale generation.

No component or source implementation is included.
