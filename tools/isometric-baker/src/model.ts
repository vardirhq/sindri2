/**
 * Declarative primitive models, and the mesh a bake actually rasterises.
 *
 * The spec types are ported from IsoGame's
 * `tools/sprite-factory/src/model.ts` (MIT). What is deliberately *not* ported
 * is everything game-specific that lived beside them there — furniture
 * categories, sit/lay interaction spots, stackability, IsoGame's collision
 * model. Those describe a social game's furniture, not a Sindri asset, and
 * putting them in a generic engine format would make every Sindri project carry
 * another game's vocabulary.
 *
 * Coordinate conventions for a model:
 *   - centred on its own footprint in X/Z
 *   - y = 0 is the floor
 *   - one unit is one tile edge
 *   - it faces +X at rotation 0, which this baker calls "south"
 */

import {
  type Geometry,
  boxGeometry,
  cylinderGeometry,
  plateGeometry,
  sphereGeometry,
} from './primitives.ts';
import { type BandedMaterial, type MaterialSpec, bandMaterial } from './shading.ts';
import { type Mat3, type Vec3, IDENTITY, rotationXYZ, transform, vec } from './vec.ts';

interface PartBase {
  /** Centre of the part, in tile units. Defaults to the model's origin. */
  position?: Vec3;
  /** Euler rotation in degrees, applied X then Y then Z. */
  rotation?: Vec3;
  /** A name from the model's material table, or an inline material. */
  material: string | MaterialSpec;
}

export interface BoxPart extends PartBase {
  type: 'box';
  size: Vec3;
}

/** A flat quad in the XZ plane: a floor tile, a rug, a painted marking. */
export interface PlatePart extends PartBase {
  type: 'plate';
  /** Extent along x and z. */
  size: [number, number];
}

export interface CylinderPart extends PartBase {
  type: 'cylinder';
  radius: number;
  /** Defaults to `radius`; set separately for a tapered shape. */
  radiusTop?: number;
  height: number;
  segments?: number;
}

export interface SpherePart extends PartBase {
  type: 'sphere';
  radius: number;
  segments?: number;
  /** Non-uniform squash, applied after the sphere is built. */
  scale?: Vec3;
}

export interface ConePart extends PartBase {
  type: 'cone';
  radius: number;
  height: number;
  segments?: number;
}

export type ModelPart = BoxPart | PlatePart | CylinderPart | SpherePart | ConePart;

const DEFAULT_SEGMENTS = 16;

function geometryOf(part: ModelPart): Geometry {
  switch (part.type) {
    case 'box':
      return boxGeometry(part.size);
    case 'plate':
      return plateGeometry(part.size);
    case 'cylinder':
      return cylinderGeometry({
        radiusTop: part.radiusTop ?? part.radius,
        radiusBottom: part.radius,
        height: part.height,
        segments: part.segments ?? DEFAULT_SEGMENTS,
      });
    case 'sphere':
      return sphereGeometry({
        radius: part.radius,
        segments: part.segments ?? DEFAULT_SEGMENTS,
        scale: part.scale,
      });
    case 'cone':
      return cylinderGeometry({
        radiusTop: 0,
        radiusBottom: part.radius,
        height: part.height,
        segments: part.segments ?? DEFAULT_SEGMENTS,
      });
  }
}

/**
 * A whole model flattened into one triangle soup.
 *
 * Flat typed arrays rather than a scene graph: the rasteriser walks every
 * triangle of every direction, and a model small enough to read at 64 pixels is
 * small enough that transforming it four times costs nothing worth a hierarchy.
 */
export interface Mesh {
  positions: Float64Array;
  normals: Float64Array;
  indices: Uint32Array;
  /** Which entry of `materials` each triangle is drawn with. */
  triangleMaterial: Uint16Array;
  materials: BandedMaterial[];
  /** Every colour a pixel of this model can be, in the order they were declared. */
  palette: string[];
  /** Named material to the ramp it was banded with, for the bake report. */
  ramps: Record<string, string[]>;
}

export interface Bounds {
  min: Vec3;
  max: Vec3;
}

export function boundsOf(mesh: Mesh, rotation: Mat3 = IDENTITY): Bounds {
  let [minX, minY, minZ] = [Infinity, Infinity, Infinity];
  let [maxX, maxY, maxZ] = [-Infinity, -Infinity, -Infinity];

  for (let i = 0; i < mesh.positions.length; i += 3) {
    const p = transform(rotation, vec(mesh.positions[i], mesh.positions[i + 1], mesh.positions[i + 2]));
    minX = Math.min(minX, p[0]);
    minY = Math.min(minY, p[1]);
    minZ = Math.min(minZ, p[2]);
    maxX = Math.max(maxX, p[0]);
    maxY = Math.max(maxY, p[1]);
    maxZ = Math.max(maxZ, p[2]);
  }

  if (!Number.isFinite(minX)) {
    return { min: vec(0, 0, 0), max: vec(0, 0, 0) };
  }
  return { min: vec(minX, minY, minZ), max: vec(maxX, maxY, maxZ) };
}

export interface ModelSpec {
  materials: Record<string, MaterialSpec>;
  parts: ModelPart[];
}

/**
 * Build a model's mesh, resolving every part's material to its four shades.
 *
 * Materials are resolved once and shared, so the palette lists each ramp once
 * however many parts use it — which matters because the palette's order decides
 * how a tie is broken when a blended edge pixel is snapped back onto it.
 */
export function buildMesh(spec: ModelSpec): Mesh {
  const positions: number[] = [];
  const normals: number[] = [];
  const indices: number[] = [];
  const triangleMaterial: number[] = [];
  const materials: BandedMaterial[] = [];
  const palette: string[] = [];
  const ramps: Record<string, string[]> = {};
  const cache = new Map<string, number>();

  const resolve = (ref: string | MaterialSpec): number => {
    const key = typeof ref === 'string' ? ref : JSON.stringify(ref);
    const cached = cache.get(key);
    if (cached !== undefined) return cached;

    const materialSpec = typeof ref === 'string' ? spec.materials[ref] : ref;
    if (!materialSpec) throw new Error(`unknown material ${JSON.stringify(ref)}`);

    const banded = bandMaterial(materialSpec);
    const index = materials.length;
    materials.push(banded);
    palette.push(...banded.ramp);
    if (typeof ref === 'string') ramps[ref] = banded.ramp;
    cache.set(key, index);
    return index;
  };

  for (const part of spec.parts) {
    const material = resolve(part.material);
    const geometry = geometryOf(part);
    const rotation = part.rotation ? rotationXYZ(part.rotation) : IDENTITY;
    const offset = part.position ?? vec(0, 0, 0);
    const base = positions.length / 3;

    for (let i = 0; i < geometry.positions.length; i += 3) {
      const local = vec(geometry.positions[i], geometry.positions[i + 1], geometry.positions[i + 2]);
      const normal = vec(geometry.normals[i], geometry.normals[i + 1], geometry.normals[i + 2]);
      const world = transform(rotation, local);
      const rotatedNormal = transform(rotation, normal);
      positions.push(world[0] + offset[0], world[1] + offset[1], world[2] + offset[2]);
      normals.push(rotatedNormal[0], rotatedNormal[1], rotatedNormal[2]);
    }

    for (let i = 0; i < geometry.indices.length; i += 3) {
      indices.push(
        base + geometry.indices[i],
        base + geometry.indices[i + 1],
        base + geometry.indices[i + 2],
      );
      triangleMaterial.push(material);
    }
  }

  if (materials.length > 0xffff) {
    throw new Error(`a model may use at most 65536 materials, got ${materials.length}`);
  }

  return {
    positions: Float64Array.from(positions),
    normals: Float64Array.from(normals),
    indices: Uint32Array.from(indices),
    triangleMaterial: Uint16Array.from(triangleMaterial),
    materials,
    palette,
    ramps,
  };
}
