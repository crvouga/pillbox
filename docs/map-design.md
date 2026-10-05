# Grid-built arenas

Every newly authored map uses one construction kit. X and Z snap to 4 m cells;
base heights snap to 1 m increments. Pieces rotate in 90° turns. The anchor is
the lower world-space corner, so rotating or duplicating a piece preserves exact
face alignment. There is no arbitrary scaling, free sphere placement, decorative
model placement, architectural trim, or painted floor grid.

| Piece | Width × height × depth |
| --- | --- |
| Square box | 4 × 4 × 4 m |
| Rectangle box | 4 × 4 × 8 m |
| Square slab | 4 × 1 × 4 m |
| Rectangle slab | 4 × 1 × 8 m |
| Sphere | 4 × 4 × 4 m bounding cell |
| Short ramp | 4 × 4 × 8 m |
| Long ramp | 4 × 4 × 16 m |

Combine slabs for low cover, boxes for walls and supports, and paired ramps for
8 m-wide routes. Floors have their top at the walking level: a ground slab has
base Y = −1; an 8 m deck has base Y = 7. Supports terminate at the deck's lower
face. Pieces may touch, but their bounding volumes cannot interpenetrate.

Gameplay markers use a separate palette and fixed grid footprints. Spawns,
flags, bases and portal destinations denote walking-surface points. Supplies,
zones and pads use their catalog's vertical offsets. The server exports both
catalogs to the editor and validates dimensions, anchors, rotations and overlaps
before saving. Setting or omitting `grid` to avoid validation is rejected for
new map documents. Historical published versions remain readable; the editor
keeps those free-form versions read-only rather than silently moving geometry.

## Arenas

All three arenas have a 64 × 80 m usable ground footprint, 8 m decks and 16 m
opposing overlooks. Intermediate landings sit at 4 and 12 m. Two opposite,
8 m-wide ramp routes connect every main level. The middle decks leave 7 m of
clearance below them. Ground cover interrupts firing lanes; supports provide
cover beneath the decks. Supplies reward climbing. Structural surfaces use a
consistent matte palette and shadowed daylight, without decorative geometry.

- **Terraces:** one broad elevated crossing through the center.
- **Crosscut:** two separated elevated crossings, longer ground approach ramps,
  and a hill on the open ground between them.
- **Courtyard:** an elevated ring with a central connection and two floor openings.

The sandbox shows the active build height and ghosts other layers. Enable
“Show / select every level” to inspect the entire structure. Object dimensions
are read-only. Duplicate advances by the rotated piece's full width. The 3D
preview uses a wider view of the new arenas. “New” starts with tiled ground and
spawns; all remaining structures are built from the kit.

## Verification

Regression tests walk the actual Box3D controller up both routes to 8 and 16 m
on every arena, check supported spawns and underpass headroom, and require bot
routes to end on the correct surface beside every power weapon. Kit tests cover
all seven pieces and four rotations, embedded-capsule clearance, serialization,
off-grid positions, resizing, free sphere movement and overlapping geometry.
HTTP acceptance rejects hostile attempts to bypass the catalog. Editor tests
exercise the server's catalog and face-aligned rotation/duplication. Native GPU
captures render all three maps and the first-person view using the client scene.
