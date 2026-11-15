---
name: molecule
description: Show a protein, DNA or small-molecule structure in 3D with dataviz.
argument-hint: "<PDB id or .pdb file> [style] [rotation]"
disable-model-invocation: true
---

Show the structure: $ARGUMENTS

Call the dataviz `molecule` tool: a 4-character id goes to `pdb_id`, a file
path to `pdb_path`. Use the style the request names (cartoon,
ball_and_stick, sticks, space_filling, backbone; default: cartoon for
proteins) and coloring (element, chain, rainbow). Turn it with
`rotate: {"x": …, "y": …, "z": …}` when a view is asked for. Add
`ramachandran: true` for backbone geometry questions. Then describe the
structure briefly (chains, helices and strands visible, ligands).
