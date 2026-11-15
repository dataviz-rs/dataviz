# 3D

All 3D series of a plot share one scene — one box, one camera, one depth
order — so they hide each other correctly. Faces are lit, spheres and tubes
shaded, far parts can fade (`depth_cue`).

```rust,ignore
ax.surface_fn(|x, y| (x * y).sin(), (-3.0, 3.0), (-3.0, 3.0), 60);
ax.scatter3d(&x, &y, &z).values(&v).colormap(Colormap::Turbo);
ax.line3d(&x, &y, &z);
ax.spheres3d(&x, &y, &z, [0.5]);
ax.ellipsoid([0.0, 0.0, 0.0], [3.0, 2.0, 1.0]).opacity(0.2);
ax.isosurface(&volume, [nx, ny, nz], bounds, level);
ax.quiver3d(points, vectors);
ax.bar3d(columns, rows, values);
ax.camera(-60.0, 30.0).rotate(Axis3::X, 20.0).roll(5.0).perspective(0.4);
```

## Molecules and DNA

```rust,ignore
ax.dna_helix("ATGCGTATAAAAGCC").highlight(5..11, "#ffd400", Some("TATA box"));
let ubq = Molecule::from_pdb(&std::fs::read_to_string("1ubq.pdb")?);
ax.molecule(ubq.clone()).style(MolStyle::Cartoon);       // helices, strands
let angles = ubq.phi_psi();                              // Ramachandran
```

Styles: ball-and-stick, sticks, space-filling, backbone, cartoon; colors by
element, chain or along the chain. Structures open with their longest
extent across the screen.

In windows, HTML pages and apps the scenes turn with the mouse (see
[Interactivity](interactive.md)).
