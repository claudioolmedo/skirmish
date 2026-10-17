// Skirmish II minimap — standalone module for the play page.
//
// Pure functions only: no DOM access, no sim access. The caller (app.js)
// feeds it a plain state snapshot each frame and a 2D context; all
// coordinate math lives here so it can be unit-tested headlessly
// (see minimap.test.mjs).
//
// The minimap is a DIAMOND using the same 2:1 dimetric projection as
// the main view (world x-y -> u, (x+y)/2 -> v), so it reads as a small
// copy of the world rather than a rotated square. A useful consequence
// of sharing the projection: the camera viewport is an AXIS-ALIGNED
// rectangle on the minimap (see viewportRect).
//
// World model (mirrors sim/src/grid.rs): the world is the square
// [-worldHalf, worldHalf) on both axes; the grid is gridW x gridH
// cells of cellSize world units; obstacles is a flat row-major array
// indexed gy * gridW + gx, truthy = blocked.

/**
 * Create the minimap geometry for a canvas of `width` x `height` px.
 * Fits the whole world diamond inside, centered, with `pad` px slack.
 */
export function createMinimap({ worldHalf, width, height, pad = 2 }) {
  if (!(worldHalf > 0) || !(width > 0) || !(height > 0)) {
    throw new Error('createMinimap: worldHalf, width and height must be positive');
  }
  // Iso extents: u = wx - wy spans [-2H, 2H]; v = (wx + wy)/2 spans [-H, H].
  const spanU = 4 * worldHalf;
  const spanV = 2 * worldHalf;
  const scale = Math.min((width - 2 * pad) / spanU, (height - 2 * pad) / spanV);
  return { worldHalf, width, height, scale, cx: width / 2, cy: height / 2 };
}

/** World point -> minimap pixel [mx, my]. */
export function worldToMinimap(g, wx, wy) {
  return [
    g.cx + (wx - wy) * g.scale,
    g.cy + ((wx + wy) / 2) * g.scale,
  ];
}

/** Minimap pixel -> world point [wx, wy] (exact inverse of worldToMinimap). */
export function minimapToWorld(g, mx, my) {
  const u = (mx - g.cx) / g.scale;
  const v = (my - g.cy) / g.scale;
  return [v + u / 2, v - u / 2];
}

/**
 * The camera viewport as an axis-aligned rect in minimap pixels.
 * `cam` mirrors app.js camera state: { s, ox, oy, viewW, viewH } where
 * (ox, oy) is the screen position of world (0,0) and s is screen px
 * per iso unit. In iso space the visible region is the rect
 * u in [-ox/s, (viewW-ox)/s], v in [-oy/s, (viewH-oy)/s] — axis-aligned
 * because the minimap shares the main view's projection.
 * The rect may extend past the diamond (screen corners can see outside
 * the world); renderMinimap clips it to the diamond.
 */
export function viewportRect(g, cam) {
  const u0 = -cam.ox / cam.s;
  const u1 = (cam.viewW - cam.ox) / cam.s;
  const v0 = -cam.oy / cam.s;
  const v1 = (cam.viewH - cam.oy) / cam.s;
  return {
    x: g.cx + u0 * g.scale,
    y: g.cy + v0 * g.scale,
    w: (u1 - u0) * g.scale,
    h: (v1 - v0) * g.scale,
  };
}

/**
 * Camera offsets [ox, oy] that center the main view on world (wx, wy)
 * at zoom `s` — the same math as app.js centerCamera. Minimap clicks
 * use this: minimapToWorld, then cameraCenteredOn.
 */
export function cameraCenteredOn(wx, wy, s, viewW, viewH) {
  return [
    viewW / 2 - (wx - wy) * s,
    viewH / 2 - ((wx + wy) / 2) * s,
  ];
}

/** Path the world diamond on `ctx` (does not fill or stroke it). */
export function diamondPath(ctx, g) {
  const H = g.worldHalf;
  const [tx, ty] = worldToMinimap(g, -H, -H); // top corner
  const [rx, ry] = worldToMinimap(g, H, -H); // right corner
  const [bx, by] = worldToMinimap(g, H, H); // bottom corner
  const [lx, ly] = worldToMinimap(g, -H, H); // left corner
  ctx.beginPath();
  ctx.moveTo(tx, ty);
  ctx.lineTo(rx, ry);
  ctx.lineTo(bx, by);
  ctx.lineTo(lx, ly);
  ctx.closePath();
}

// Node colors by sim NodeKind tag (0 tree, 1 berry, 2 gold, 3 stone,
// 4 farm, 5 hunt carcass, 6 herd carcass — M3). Our own minimap
// palette — flat and readable at 2 px.
export const NODE_COLORS = ['#2e7d32', '#e0497e', '#ffd23e', '#a8adb5', '#86c860', '#b5542c', '#c97b4a'];
export const GROUND_COLOR = '#44512f'; // muted moss — the unexplored-later base
export const OBSTACLE_COLOR = '#232a20'; // wall cells read as dark blocks
export const BUILDING_COLOR = '#e8e3d5'; // own buildings when no color is given
export const VIEWPORT_COLOR = 'rgba(255, 255, 255, 0.9)';
export const SELECTED_COLOR = '#ffcf4d'; // matches the main view's selection gold

function drawDiamond(ctx, g, cxWorld, cyWorld, halfWorld, fill) {
  // A world-axis-aligned square (center + half extent) projected to
  // the minimap: its four corners form the drawn diamond.
  const pts = [
    worldToMinimap(g, cxWorld - halfWorld, cyWorld - halfWorld),
    worldToMinimap(g, cxWorld + halfWorld, cyWorld - halfWorld),
    worldToMinimap(g, cxWorld + halfWorld, cyWorld + halfWorld),
    worldToMinimap(g, cxWorld - halfWorld, cyWorld + halfWorld),
  ];
  ctx.fillStyle = fill;
  ctx.beginPath();
  ctx.moveTo(pts[0][0], pts[0][1]);
  for (let i = 1; i < 4; i++) ctx.lineTo(pts[i][0], pts[i][1]);
  ctx.closePath();
  ctx.fill();
}

/**
 * Render one minimap frame.
 *
 * state = {
 *   gridW, gridH, cellSize,
 *   obstacles: flat row-major array (truthy = blocked cell),
 *   nodes:     [{ x, y, kind, amount }],          // amount <= 0 skipped
 *   buildings: [{ x, y, kind, complete, color?, size? }], // size = footprint
 *                        // edge in cells (2 or 3); default 2 (M3)
 *   entities:  [{ x, y, kind, color, selected? }],  // kind: 0 villager, 1 TC,
 *                        // 2 scout, 3 militia, 4 archer, 5+ animals
 *   cam:       { s, ox, oy, viewW, viewH } | null,  // null = no viewport rect
 * }
 *
 * Draw order: ground diamond, obstacle cells, nodes, buildings,
 * entities, viewport rect — everything after the ground is clipped to
 * the diamond, then the diamond border is stroked on top.
 */
export function renderMinimap(ctx, g, state) {
  const H = g.worldHalf;
  const { gridW, gridH, cellSize, obstacles } = state;

  // Ground.
  diamondPath(ctx, g);
  ctx.fillStyle = GROUND_COLOR;
  ctx.fill();

  ctx.save();
  diamondPath(ctx, g);
  ctx.clip();

  // Obstacle cells as dark diamonds.
  if (obstacles) {
    ctx.fillStyle = OBSTACLE_COLOR;
    for (let gy = 0; gy < gridH; gy++) {
      for (let gx = 0; gx < gridW; gx++) {
        if (!obstacles[gy * gridW + gx]) continue;
        const wx = -H + (gx + 0.5) * cellSize;
        const wy = -H + (gy + 0.5) * cellSize;
        drawDiamond(ctx, g, wx, wy, cellSize / 2, OBSTACLE_COLOR);
      }
    }
  }

  // Resource nodes as colored dots.
  const nodeR = Math.max(1.1, cellSize * g.scale * 0.32);
  for (const n of state.nodes || []) {
    if (n.amount <= 0) continue;
    const [mx, my] = worldToMinimap(g, n.x, n.y);
    ctx.fillStyle = NODE_COLORS[n.kind] || '#ffffff';
    ctx.beginPath();
    ctx.arc(mx, my, nodeR, 0, Math.PI * 2);
    ctx.fill();
  }

  // Buildings as footprint diamonds (2x2 cells in the sim; the
  // Barracks is 3x3 — `size` carries the footprint edge, default 2).
  // Complete buildings are solid; under construction they are hollow.
  for (const b of state.buildings || []) {
    if (b.kind === 1 && b.complete) continue; // a finished farm reads as a node
    const color = b.color || BUILDING_COLOR;
    const half = (cellSize * (b.size || 2)) / 2;
    if (b.complete) {
      drawDiamond(ctx, g, b.x, b.y, half, color);
    } else {
      const pts = [
        worldToMinimap(g, b.x - half, b.y - half),
        worldToMinimap(g, b.x + half, b.y - half),
        worldToMinimap(g, b.x + half, b.y + half),
        worldToMinimap(g, b.x - half, b.y + half),
      ];
      ctx.strokeStyle = color;
      ctx.lineWidth = 1;
      ctx.beginPath();
      ctx.moveTo(pts[0][0], pts[0][1]);
      for (let i = 1; i < 4; i++) ctx.lineTo(pts[i][0], pts[i][1]);
      ctx.closePath();
      ctx.stroke();
    }
  }

  // Entities: villagers are dots in their player color; the Town
  // Center is a larger square. Selected units get a gold underlay.
  for (const e of state.entities || []) {
    const [mx, my] = worldToMinimap(g, e.x, e.y);
    if (e.selected) {
      ctx.fillStyle = SELECTED_COLOR;
      ctx.beginPath();
      ctx.arc(mx, my, e.kind === 1 ? 4.6 : 3.2, 0, Math.PI * 2);
      ctx.fill();
    }
    ctx.fillStyle = e.color || '#ffffff';
    if (e.kind === 1) {
      ctx.fillRect(mx - 3, my - 3, 6, 6);
    } else {
      ctx.beginPath();
      ctx.arc(mx, my, 1.9, 0, Math.PI * 2);
      ctx.fill();
    }
  }

  // Camera viewport rectangle.
  if (state.cam) {
    const r = viewportRect(g, state.cam);
    ctx.strokeStyle = VIEWPORT_COLOR;
    ctx.lineWidth = 1.5;
    ctx.strokeRect(r.x, r.y, r.w, r.h);
  }

  ctx.restore();

  // Diamond border.
  diamondPath(ctx, g);
  ctx.strokeStyle = 'rgba(232, 227, 213, 0.55)';
  ctx.lineWidth = 1;
  ctx.stroke();
}
