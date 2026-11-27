/**
 * Snell's-law slab refraction map generator adapted from Glassy UI.
 * Computes a rounded-rectangle edge displacement field (R/G = inward bend,
 * B = rim brightness mask) as a cached PNG data URL for Chromium SVG
 * backdrop-filter lenses.
 */

export type LensRadii = readonly [number, number, number, number];

export type LensMapOptions = {
  bevel: number;
  depth: number;
  devicePixelRatio?: number;
  height: number;
  ior: number;
  radii: LensRadii;
  thickness: number;
  width: number;
};

export type LensMapMetrics = {
  bevel: number;
  cacheKey: string;
  height: number;
  mapHeight: number;
  mapWidth: number;
  reach: number;
  width: number;
};

export type LensMapBuildResult = {
  href: string;
  metrics: LensMapMetrics;
};

// Glass slab optical constants (match Glassy UI slab-optics.ts).
const SLAB_AIR_GAP = 8;
const SLAB_MIN_RISE = 0.3;
const SLAB_RISE_SPAN = 0.7;
const SLAB_MAX_SLOPE = 12;

const MAX_LENS_MAP_PIXELS = 240_000;
export const MAX_LENS_SURFACE_AREA = 200_000;

const LENS_REACH = 0.7;
const LENS_FOLD = 0.75;
const LENS_PROFILE_STEPS = 512;

const LENS_RIM_LIGHT = 0.8;
const LENS_RIM_LIGHT_SURFACE = 0.16;
const LENS_RIM_LIGHT_FALLOFF = 0.45;

const lensMapCache = new Map<string, LensMapBuildResult>();
const MAX_CACHED_LENS_MAPS = 48;

const FLAT_LENS_PIXEL = new Uint32Array(new Uint8Array([128, 128, 0, 255]).buffer)[0];

type Edge = { distance: number; normalX: number; normalY: number };

export function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

function smoothstep(edge0: number, edge1: number, value: number): number {
  const t = clamp((value - edge0) / Math.max(edge1 - edge0, 0.0001), 0, 1);
  return t * t * (3 - 2 * t);
}

/**
 * Computes inward ray displacement across a curved glass rim using Snell's law.
 * The slab top profile follows (1 - (1 - u)^4)^(1/4) from rim edge to flat top.
 */
function slabOffset(s: number, bevel: number, thickness: number, ior: number): number {
  const u = s / bevel;
  const v = 1 - Math.max(u, 0.002);
  let rise = 1;
  let steepness = 0;

  if (u < 1) {
    const v4 = v ** 4;
    rise = (1 - v4) ** 0.25;
    steepness = v ** 3 * Math.max(1 - v4, 1e-6) ** -0.75;
  }

  const slope = Math.min(((SLAB_RISE_SPAN * thickness) / bevel) * steepness, SLAB_MAX_SLOPE);
  const height = thickness * (SLAB_MIN_RISE + SLAB_RISE_SPAN * rise);
  const tilt = Math.hypot(slope, 1);
  const sinTilt = slope / tilt;
  const cosTilt = 1 / tilt;
  const eta = 1 / ior;
  const root = Math.sqrt(Math.max(1 - eta * eta * sinTilt * sinTilt, 0));
  const bend = eta * cosTilt - root;
  const along = bend * sinTilt;
  const down = Math.max(eta - bend * cosTilt, 0.05);
  const sinOut = ior * Math.abs(along);
  const cosOut = Math.sqrt(Math.max(1 - sinOut * sinOut, 0));
  const leaving = (ior * along) / Math.max(sinOut, 1);

  return -(along * (height / down) + leaving * (SLAB_AIR_GAP / Math.max(cosOut, 0.1)));
}

function lensProfile(bevel: number, thickness: number, ior: number, depth: number) {
  const cap = LENS_REACH * bevel;
  const step = bevel / LENS_PROFILE_STEPS;
  const offsets = new Float32Array(LENS_PROFILE_STEPS + 1);

  for (let i = 0; i < LENS_PROFILE_STEPS; i += 1) {
    offsets[i] = cap * Math.tanh((depth * slabOffset(i * step, bevel, thickness, ior)) / cap);
  }

  // Enforce monotonic compression so the rim never folds/mirrors back on itself.
  for (let i = LENS_PROFILE_STEPS - 1; i >= 0; i -= 1) {
    offsets[i] = Math.min(offsets[i], offsets[i + 1] + LENS_FOLD * step);
  }

  let reach = 0.01;
  for (let i = 0; i < offsets.length; i += 1) {
    if (offsets[i] > reach) reach = offsets[i];
  }

  return { offsets, reach };
}

function roundedBoxEdge(
  x: number,
  y: number,
  width: number,
  height: number,
  radii: LensRadii,
  edge: Edge,
): Edge {
  const px = x - width / 2;
  const py = y - height / 2;
  const radius = px < 0 ? (py < 0 ? radii[0] : radii[3]) : py < 0 ? radii[1] : radii[2];
  const qx = Math.abs(px) - (width / 2 - radius);
  const qy = Math.abs(py) - (height / 2 - radius);
  const signX = px < 0 ? -1 : 1;
  const signY = py < 0 ? -1 : 1;

  if (qx > 0 && qy > 0) {
    const length = Math.hypot(qx, qy);
    edge.distance = length - radius;
    edge.normalX = (signX * qx) / length;
    edge.normalY = (signY * qy) / length;
  } else if (qx > qy) {
    edge.distance = qx - radius;
    edge.normalX = signX;
    edge.normalY = 0;
  } else {
    edge.distance = qy - radius;
    edge.normalX = 0;
    edge.normalY = signY;
  }

  return edge;
}

function computeLensMapMetrics(options: LensMapOptions) {
  const width = Math.max(1, options.width);
  const height = Math.max(1, options.height);
  const bevel = clamp(options.bevel, 1, Math.min(width, height) / 2);
  const pixelRatio = clamp(options.devicePixelRatio ?? 1, 1, 2);
  let mapWidth = clamp(Math.round(width * pixelRatio), 32, 1800);
  let mapHeight = clamp(Math.round(height * pixelRatio), 32, 1200);

  if (mapWidth * mapHeight > MAX_LENS_MAP_PIXELS) {
    const ratio = Math.sqrt(MAX_LENS_MAP_PIXELS / (mapWidth * mapHeight));
    mapWidth = Math.max(32, Math.floor(mapWidth * ratio));
    mapHeight = Math.max(32, Math.floor(mapHeight * ratio));
  }

  const cacheKey = [
    mapWidth,
    mapHeight,
    Math.round(width),
    Math.round(height),
    ...options.radii.map(r => Math.round(r * 10)),
    Math.round(bevel * 10),
    Math.round(options.thickness * 10),
    Math.round(options.ior * 100),
    Math.round(options.depth * 100),
  ].join(":");

  return { bevel, cacheKey, height, mapHeight, mapWidth, width };
}

export function buildLensMapDataUrl(options: LensMapOptions): LensMapBuildResult | null {
  const base = computeLensMapMetrics(options);
  const cached = lensMapCache.get(base.cacheKey);
  if (cached) {
    lensMapCache.delete(base.cacheKey);
    lensMapCache.set(base.cacheKey, cached);
    return cached;
  }

  if (typeof document === "undefined") return null;

  const canvas = document.createElement("canvas");
  canvas.width = base.mapWidth;
  canvas.height = base.mapHeight;
  const ctx = canvas.getContext("2d");
  if (!ctx) return null;

  const { bevel, height, mapHeight, mapWidth, width } = base;
  const { offsets, reach } = lensProfile(bevel, options.thickness, options.ior, options.depth);

  const litTo = Math.min(LENS_RIM_LIGHT, (LENS_RIM_LIGHT_SURFACE * Math.min(width, height)) / bevel);
  const litFrom = litTo * LENS_RIM_LIGHT_FALLOFF;
  const data = new Uint8ClampedArray(mapWidth * mapHeight * 4);
  const pixels = new Uint32Array(data.buffer);
  const edge: Edge = { distance: 0, normalX: 0, normalY: 0 };

  const flatInset = Math.max(bevel, ...options.radii);
  const flatLeft = Math.min(mapWidth, Math.ceil((flatInset / width) * mapWidth + 0.5));
  const flatRight = Math.max(flatLeft, Math.floor(((width - flatInset) / width) * mapWidth - 0.5));
  const flatTop = Math.ceil((bevel / height) * mapHeight + 0.5);
  const flatBottom = Math.floor(((height - bevel) / height) * mapHeight - 0.5);

  for (let yIndex = 0; yIndex < mapHeight; yIndex += 1) {
    const y = ((yIndex + 0.5) / mapHeight) * height;
    const flatRow = yIndex >= flatTop && yIndex < flatBottom;

    for (let xIndex = 0; xIndex < mapWidth; xIndex += 1) {
      if (flatRow && xIndex === flatLeft && flatRight > flatLeft) {
        pixels.fill(FLAT_LENS_PIXEL, yIndex * mapWidth + flatLeft, yIndex * mapWidth + flatRight);
        xIndex = flatRight - 1;
        continue;
      }

      const x = ((xIndex + 0.5) / mapWidth) * width;
      const index = (yIndex * mapWidth + xIndex) * 4;
      roundedBoxEdge(x, y, width, height, options.radii, edge);
      const inset = Math.max(-edge.distance, 0);
      let offset = 0;

      if (inset < bevel) {
        const position = (inset / bevel) * LENS_PROFILE_STEPS;
        const lower = Math.floor(position);
        const fraction = position - lower;
        offset = offsets[lower] * (1 - fraction) + offsets[lower + 1] * fraction;
      }

      data[index] = Math.round(clamp(0.5 - (offset * edge.normalX) / (2 * reach), 0, 1) * 255);
      data[index + 1] = Math.round(clamp(0.5 - (offset * edge.normalY) / (2 * reach), 0, 1) * 255);
      data[index + 2] = Math.round((1 - smoothstep(litFrom, litTo, inset / bevel)) * 255);
      data[index + 3] = 255;
    }
  }

  ctx.putImageData(new ImageData(data, mapWidth, mapHeight), 0, 0);
  const result: LensMapBuildResult = {
    href: canvas.toDataURL("image/png"),
    metrics: { ...base, reach },
  };

  lensMapCache.set(base.cacheKey, result);
  if (lensMapCache.size > MAX_CACHED_LENS_MAPS) {
    const oldestKey = lensMapCache.keys().next().value;
    if (oldestKey) lensMapCache.delete(oldestKey);
  }

  return result;
}
