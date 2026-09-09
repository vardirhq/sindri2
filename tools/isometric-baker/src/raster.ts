/**
 * The software rasteriser.
 *
 * This is the one stage of IsoGame's Sprite Factory that is not ported: there,
 * `render.ts` builds a Three.js scene and reads pixels back out of a WebGL
 * render target, which needs a browser, and the headless exporter therefore
 * needs Vite and Playwright and a Chromium with a working GL context.
 *
 * Sindri wants baked assets to be *checked in* and regression-tested
 * byte-for-byte, and a GPU is the wrong thing to make that promise on: driver,
 * ANGLE backend and GPU model all change what comes back from
 * `readRenderTargetPixels`. Everything this pipeline asks a renderer to do is
 * flat-shaded triangles under an orthographic camera with a depth buffer, which
 * is small enough to do exactly, on the CPU, in the same arithmetic on every
 * machine. So the baker does.
 *
 * What is kept identical to the GPU path is what the pixels mean: the same
 * orthographic rig from `iso.ts`, the same banded shading from `shading.ts`,
 * and the same supersample-then-downsample order.
 */

import { type IsoCamera, depthOf, projectToPixels } from './iso.ts';
import { type RgbaImage, createImage } from './image.ts';
import { type Mesh } from './model.ts';
import { type ShadingConfig, lightVector, shadeIndex } from './shading.ts';
import { type Mat3, type Vec3, transform, vec } from './vec.ts';

export interface RasterRequest {
  mesh: Mesh;
  /** Applied to the model before projection: this is the frame's direction. */
  rotation: Mat3;
  camera: IsoCamera;
  /** Canvas size in *final* pixels; the raster is this times `supersample`. */
  width: number;
  height: number;
  supersample: number;
  /** The world point that lands on the exact centre of the canvas. */
  origin: Vec3;
  shading: ShadingConfig;
}

/**
 * Draw one frame at supersampled resolution.
 *
 * The canvas is symmetric about `origin` by construction, so `origin` projects
 * to the exact centre of the image. That is what makes a baked sprite's anchor
 * an integer rather than something to round and then apologise for.
 */
export function rasterise(request: RasterRequest): RgbaImage {
  const { mesh, rotation, camera, supersample, origin, shading } = request;
  const width = Math.round(request.width * supersample);
  const height = Math.round(request.height * supersample);
  const image = createImage(width, height);
  const depth = new Float64Array(width * height).fill(-Infinity);

  const light = lightVector(shading);
  const vertexCount = mesh.positions.length / 3;
  const screenX = new Float64Array(vertexCount);
  const screenY = new Float64Array(vertexCount);
  const screenZ = new Float64Array(vertexCount);
  const worldNormals = new Float64Array(mesh.normals.length);

  const centreX = (request.width / 2) * supersample;
  const centreY = (request.height / 2) * supersample;

  for (let v = 0; v < vertexCount; v++) {
    const i = v * 3;
    const point = transform(
      rotation,
      vec(mesh.positions[i], mesh.positions[i + 1], mesh.positions[i + 2]),
    );
    const pixel = projectToPixels(camera, point, origin);
    screenX[v] = centreX + pixel.x * supersample;
    screenY[v] = centreY + pixel.y * supersample;
    screenZ[v] = depthOf(camera, point);

    const normal = transform(rotation, vec(mesh.normals[i], mesh.normals[i + 1], mesh.normals[i + 2]));
    worldNormals[i] = normal[0];
    worldNormals[i + 1] = normal[1];
    worldNormals[i + 2] = normal[2];
  }

  for (let t = 0; t < mesh.triangleMaterial.length; t++) {
    drawTriangle(
      { image, depth, width, height },
      { screenX, screenY, screenZ, worldNormals },
      mesh,
      t,
      light,
      shading.thresholds,
    );
  }

  return image;
}

interface Target {
  image: RgbaImage;
  depth: Float64Array;
  width: number;
  height: number;
}

interface Projected {
  screenX: Float64Array;
  screenY: Float64Array;
  screenZ: Float64Array;
  worldNormals: Float64Array;
}

/**
 * Fill one triangle, depth-tested.
 *
 * Back faces are drawn rather than culled. A model built from separate
 * primitives is not one closed surface — an open cylinder wall, a part sunk
 * halfway into another — so culling would punch holes that only appear from
 * some directions, which is exactly the kind of bug a four-frame bake hides
 * until the asset is in a scene.
 */
function drawTriangle(
  target: Target,
  projected: Projected,
  mesh: Mesh,
  triangle: number,
  light: Vec3,
  thresholds: ShadingConfig['thresholds'],
): void {
  let ia = mesh.indices[triangle * 3];
  let ib = mesh.indices[triangle * 3 + 1];
  let ic = mesh.indices[triangle * 3 + 2];

  const { screenX, screenY, screenZ, worldNormals } = projected;
  let area = edge(screenX[ia], screenY[ia], screenX[ib], screenY[ib], screenX[ic], screenY[ic]);
  if (area === 0) return;
  if (area < 0) {
    [ib, ic] = [ic, ib];
    area = -area;
  }

  const minX = Math.max(0, Math.floor(Math.min(screenX[ia], screenX[ib], screenX[ic])));
  const maxX = Math.min(target.width - 1, Math.ceil(Math.max(screenX[ia], screenX[ib], screenX[ic])));
  const minY = Math.max(0, Math.floor(Math.min(screenY[ia], screenY[ib], screenY[ic])));
  const maxY = Math.min(target.height - 1, Math.ceil(Math.max(screenY[ia], screenY[ib], screenY[ic])));
  if (minX > maxX || minY > maxY) return;

  const material = mesh.materials[mesh.triangleMaterial[triangle]];

  for (let y = minY; y <= maxY; y++) {
    const py = y + 0.5;
    for (let x = minX; x <= maxX; x++) {
      const px = x + 0.5;

      // Edge functions against each side. A sample is inside when it is on the
      // same side of all three, which after the winding fix above means all
      // three are non-negative.
      const w0 = edge(screenX[ib], screenY[ib], screenX[ic], screenY[ic], px, py);
      if (w0 < 0) continue;
      const w1 = edge(screenX[ic], screenY[ic], screenX[ia], screenY[ia], px, py);
      if (w1 < 0) continue;
      const w2 = edge(screenX[ia], screenY[ia], screenX[ib], screenY[ib], px, py);
      if (w2 < 0) continue;

      const ba = w0 / area;
      const bb = w1 / area;
      const bc = w2 / area;

      // Orthographic projection is affine, so depth interpolates linearly in
      // screen space with no perspective correction to get wrong.
      const z = ba * screenZ[ia] + bb * screenZ[ib] + bc * screenZ[ic];
      const offset = y * target.width + x;
      if (z <= target.depth[offset]) continue;
      target.depth[offset] = z;

      const shade = material.unlit
        ? 3
        : shadeIndex(
            vec(
              ba * worldNormals[ia * 3] + bb * worldNormals[ib * 3] + bc * worldNormals[ic * 3],
              ba * worldNormals[ia * 3 + 1] +
                bb * worldNormals[ib * 3 + 1] +
                bc * worldNormals[ic * 3 + 1],
              ba * worldNormals[ia * 3 + 2] +
                bb * worldNormals[ib * 3 + 2] +
                bc * worldNormals[ic * 3 + 2],
            ),
            light,
            thresholds,
          );

      const colour = material.shades[shade];
      const pixel = offset * 4;
      target.image.data[pixel] = colour.r;
      target.image.data[pixel + 1] = colour.g;
      target.image.data[pixel + 2] = colour.b;
      target.image.data[pixel + 3] = 255;
    }
  }
}

/** Twice the signed area of the triangle (ax,ay) (bx,by) (px,py). */
function edge(ax: number, ay: number, bx: number, by: number, px: number, py: number): number {
  return (bx - ax) * (py - ay) - (by - ay) * (px - ax);
}
